use proc_macro::TokenStream;
use proc_macro2::TokenTree;
use quote::{quote, quote_spanned};
use sqlparser::ast::{Expr, Query, SelectItem, Statement};
use sqlparser::dialect::AnsiDialect;
use syn::{Data, DeriveInput, Fields, Meta, parse_macro_input};

/// Compile-time SQL syntax validation and introspection for ANSI SQL.
///
/// Accepts SQL as bare tokens. Double-quoted strings are converted to single quotes.
/// Validates SQL syntax at compile time using sqlparser and extracts metadata
/// (tables, columns, query type, parameter count).
///
/// # Examples
///
/// ```rust,ignore
/// sql!(SELECT * FROM ducks WHERE name = "Donald")
/// sql!(INSERT INTO ducks (id, name) VALUES (1, "Donald Duck"))
/// ```
#[proc_macro]
pub fn sql(item: TokenStream) -> TokenStream {
    // Convert to proc_macro2 for easier manipulation
    let item2: proc_macro2::TokenStream = item.into();

    // Process tokens and build SQL string
    let sql_string = process_token_stream(item2.clone());

    // Parse and validate SQL syntax using ANSI dialect
    let statements = match sqlparser::parser::Parser::parse_sql(&AnsiDialect {}, &sql_string) {
        Ok(stmts) => stmts,
        Err(e) => {
            let first = item2.into_iter().next();
            if let Some(token) = first {
                let msg = format!("SQL syntax error: {}", e);
                return quote_spanned! { token.span() =>
                    compile_error!(#msg)
                }
                .into();
            } else {
                return quote! {
                    compile_error!("SQL syntax error")
                }
                .into();
            }
        }
    };

    // Extract metadata from the first statement
    let metadata = extract_metadata(&sql_string, &statements);

    // Generate code with full metadata
    let value = &metadata.sql;
    let tables = &metadata.tables;
    let columns = &metadata.columns;
    // Convert string query type to enum variant path
    let query_type_variant: syn::Path =
        syn::parse_str(&format!("::sqltmpl::QueryType::{}", metadata.query_type))
            .expect("Valid query type");
    let param_count = metadata.param_count;

    quote! {
        ::sqltmpl::Sql::new_with_metadata(
            #value,
            &[#(#tables),*],
            &[#(#columns),*],
            #query_type_variant,
            #param_count
        )
    }
    .into()
}

/// Metadata extracted from SQL
struct SqlMetadata {
    sql: String,
    tables: Vec<String>,
    columns: Vec<String>,
    query_type: String,
    param_count: usize,
}

fn extract_metadata(sql: &str, statements: &[Statement]) -> SqlMetadata {
    let mut tables = Vec::new();
    let mut columns = Vec::new();
    let mut query_type = "Other".to_string();
    let param_count = count_placeholders(sql);

    if let Some(stmt) = statements.first() {
        match stmt {
            Statement::Query(query) => {
                query_type = "Select".to_string();
                extract_select_metadata(query, &mut tables, &mut columns);
            }
            Statement::Insert(insert) => {
                query_type = "Insert".to_string();
                tables.push(insert.table.to_string());
            }
            Statement::Update { table, .. } => {
                query_type = "Update".to_string();
                // table is TableWithJoins, get the main table from it
                tables.push(table.relation.to_string());
            }
            Statement::Delete(delete) => {
                query_type = "Delete".to_string();
                for t in &delete.tables {
                    tables.push(t.to_string());
                }
            }
            Statement::CreateTable(create) => {
                query_type = "Create".to_string();
                tables.push(create.name.to_string());
            }
            Statement::Drop { names, .. } => {
                query_type = "Drop".to_string();
                for name in names {
                    tables.push(format_table_name(name));
                }
            }
            _ => {}
        }
    }

    SqlMetadata {
        sql: sql.to_string(),
        tables,
        columns,
        query_type,
        param_count,
    }
}

fn extract_name_from_table_factor(tf: &sqlparser::ast::TableFactor) -> Option<String> {
    match tf {
        sqlparser::ast::TableFactor::Table { name, .. } => Some(name.to_string()),
        _ => None,
    }
}

fn format_table_name(name: &sqlparser::ast::ObjectName) -> String {
    name.to_string()
}

fn extract_select_metadata(query: &Query, tables: &mut Vec<String>, columns: &mut Vec<String>) {
    if let sqlparser::ast::SetExpr::Select(select) = query.body.as_ref() {
        // Extract tables from FROM clause
        for table in &select.from {
            if let Some(name) = extract_name_from_table_factor(&table.relation) {
                tables.push(name);
            }
            // Also check for joins
            for join in &table.joins {
                if let Some(name) = extract_name_from_table_factor(&join.relation) {
                    tables.push(name);
                }
            }
        }

        // Extract columns from SELECT items
        for item in &select.projection {
            match item {
                SelectItem::UnnamedExpr(expr) => match expr {
                    Expr::Identifier(ident) => {
                        columns.push(ident.value.clone());
                    }
                    Expr::CompoundIdentifier(parts) => {
                        // table.column format - just take the column name
                        if let Some(last) = parts.last() {
                            columns.push(last.value.clone());
                        }
                    }
                    _ => {}
                },
                SelectItem::ExprWithAlias { alias, .. } => {
                    columns.push(alias.value.clone());
                }
                SelectItem::Wildcard(_) => {
                    columns.push("*".to_string());
                }
                _ => {}
            }
        }
    }
}

fn count_placeholders(sql: &str) -> usize {
    sql.matches('?').count()
}

fn process_token_stream(stream: proc_macro2::TokenStream) -> String {
    let mut sql_parts: Vec<String> = Vec::new();

    for token in stream {
        let part = match &token {
            TokenTree::Literal(lit) => {
                let repr = lit.to_string();
                // Check if it's a string literal (double quotes)
                if repr.starts_with('"') && repr.ends_with('"') && repr.len() >= 2 {
                    // Extract content and wrap in single quotes for SQL
                    let content = &repr[1..repr.len() - 1];
                    let unescaped = unescape_string(content);
                    format!("'{}'", unescaped)
                } else {
                    // Other literal (number, etc.)
                    repr
                }
            }
            TokenTree::Ident(ident) => ident.to_string(),
            TokenTree::Punct(punct) => {
                let ch = punct.as_char();
                // Don't add spaces around common SQL punctuation
                let s = ch.to_string();
                s
            }
            TokenTree::Group(group) => {
                // Recursively process group content
                let group_sql = process_token_stream(group.stream());
                let (open, close) = match group.delimiter() {
                    proc_macro2::Delimiter::Parenthesis => ("(", ")"),
                    proc_macro2::Delimiter::Brace => ("{", "}"),
                    proc_macro2::Delimiter::Bracket => ("[", "]"),
                    proc_macro2::Delimiter::None => ("", ""),
                };
                format!("{}{}{}", open, group_sql, close)
            }
        };
        sql_parts.push(part);
    }

    // Join with spaces, but be smart about it
    let mut result = String::new();
    let mut prev_was_ident_or_literal = false;

    for (i, part) in sql_parts.iter().enumerate() {
        // Check if current part is punctuation that shouldn't have space before
        let is_punct = part.len() == 1
            && part
                .chars()
                .next()
                .map(|c| !c.is_alphanumeric())
                .unwrap_or(false);
        let _is_opening = part == "(" || part == "[";
        let is_closing = part == ")" || part == "]" || part == "," || part == ";";

        // Add space if:
        // - Not first token
        // - Previous was ident/literal and current is not closing punctuation
        // - Previous was closing and current is not punctuation
        if i > 0 {
            let prev_part = &sql_parts[i - 1];
            let prev_was_closing = prev_part == ")"
                || prev_part == "]"
                || prev_part == "}"
                || prev_part == ","
                || prev_part == ";";

            let need_space = if is_closing || is_punct {
                // Don't add space before closing punctuation or operators
                false
            } else if prev_was_closing {
                // Add space after closing punctuation before next identifier/literal
                true
            } else if prev_was_ident_or_literal && !is_punct {
                // Add space between identifier and identifier/literal
                true
            } else {
                false
            };

            if need_space {
                result.push(' ');
            }
        }

        result.push_str(part);
        prev_was_ident_or_literal = !is_punct || is_closing;
    }

    result
}

/// Unescape Rust string literals for SQL
fn unescape_string(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars();

    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => result.push('\n'),
                Some('r') => result.push('\r'),
                Some('t') => result.push('\t'),
                Some('0') => result.push('\0'),
                Some('\\') => result.push('\\'),
                Some('"') => result.push('"'),
                Some('\'') => result.push('\''),
                Some(c) => {
                    result.push('\\');
                    result.push(c);
                }
                None => result.push('\\'),
            }
        } else {
            result.push(c);
        }
    }

    result
}

#[proc_macro_derive(QueryResult, attributes(from_sql))]
pub fn derive_query_result(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let struct_name = &input.ident;

    // Extract SQL from attribute if present
    let sql_from_attr = extract_sql_from_attributes(&input);

    // Generate implementations
    let generated = match &input.data {
        Data::Struct(data) => {
            let fields = match &data.fields {
                Fields::Named(named) => named.named.iter().collect::<Vec<_>>(),
                Fields::Unnamed(_) => {
                    return quote! {
                        compile_error!("QueryResult does not support tuple structs")
                    }
                    .into();
                }
                Fields::Unit => {
                    return quote! {
                        compile_error!("QueryResult does not support unit structs")
                    }
                    .into();
                }
            };

            // Generate field metadata and accessors
            let field_count = fields.len();
            let field_names: Vec<_> = fields
                .iter()
                .map(|f| f.ident.as_ref().unwrap().to_string())
                .collect();

            // Column indices
            let _column_indices: Vec<_> = (0..field_count).collect();

            // Generate from_row implementation
            let from_row_impl = {
                let field_assignments: Vec<_> = fields
                    .iter()
                    .enumerate()
                    .map(|(idx, f)| {
                        let field_name = f.ident.as_ref().unwrap();
                        let field_ty = &f.ty;
                        let idx_lit = syn::Index::from(idx);
                        quote! {
                            #field_name: row.get::<#field_ty>(#idx_lit)?,
                        }
                    })
                    .collect();

                quote! {
                    pub fn from_row(row: &impl ::sqltmpl::RowAccess) -> Result<Self, Box<dyn std::error::Error>> {
                        Ok(Self {
                            #(#field_assignments)*
                        })
                    }
                }
            };

            // Generate columns method
            let columns_impl = {
                let col_refs: Vec<_> = field_names.iter().map(|name| quote! { #name }).collect();
                quote! {
                    pub fn columns() -> &'static [&'static str] {
                        &[#(#col_refs),*]
                    }
                }
            };

            // Generate sql method that returns Sql for the query
            let sql_impl = if let Some(sql_str) = sql_from_attr {
                // Parse SQL to get metadata
                let sql_for_parsing = sql_str.replace('"', "'");
                let parsed =
                    sqlparser::parser::Parser::parse_sql(&AnsiDialect {}, &sql_for_parsing);

                let metadata = if let Ok(stmts) = parsed {
                    extract_metadata(&sql_for_parsing, &stmts)
                } else {
                    SqlMetadata {
                        sql: sql_str.clone(),
                        tables: vec![],
                        columns: field_names.clone(),
                        query_type: "Select".to_string(),
                        param_count: sql_str.matches('?').count(),
                    }
                };

                let tables = metadata.tables;
                let columns = metadata.columns;
                let query_type_str = metadata.query_type.clone();
                let query_type_variant: syn::Path =
                    syn::parse_str(&format!("::sqltmpl::QueryType::{}", query_type_str))
                        .expect("Valid query type");
                let param_count = metadata.param_count;

                quote! {
                    pub fn sql() -> ::sqltmpl::Sql {
                        ::sqltmpl::Sql::new_with_metadata(
                            #sql_str,
                            &[#(#tables),*],
                            &[#(#columns),*],
                            #query_type_variant,
                            #param_count
                        )
                    }
                }
            } else {
                quote! {
                    pub fn sql() -> ::sqltmpl::Sql {
                        ::sqltmpl::Sql::new(concat!(
                            "SELECT ",
                            #(#field_names),*,
                            " FROM ",
                            stringify!(#struct_name)
                        ))
                    }
                }
            };

            quote! {
                impl #struct_name {
                    #from_row_impl
                    #columns_impl
                    #sql_impl
                }
            }
        }
        _ => {
            return quote! {
                compile_error!("QueryResult only supports named structs")
            }
            .into();
        }
    };

    generated.into()
}

/// Extract SQL from #[from_sql(sql!(...))] attribute
fn extract_sql_from_attributes(input: &DeriveInput) -> Option<String> {
    for attr in &input.attrs {
        if attr.path().is_ident("from_sql") {
            if let Meta::List(list) = &attr.meta {
                let tokens = &list.tokens;
                let token_str = tokens.to_string();

                // Look for sql!(...) pattern in the tokens
                // The tokens will be in the form: sql ! ( SELECT ... )
                if !token_str.starts_with("sql ! (") && !token_str.starts_with("sql!(") {
                    continue;
                }

                // Extract the SQL content between parentheses
                let start = token_str.find('(').unwrap_or(0) + 1;
                let end = token_str.rfind(')').unwrap_or(token_str.len());
                let sql_content = &token_str[start..end];

                // Remove the leading space if present and trim
                let sql_content = sql_content.trim_start();

                // Process the SQL to convert Rust string literals to SQL format
                return Some(process_derive_sql(sql_content));
            }
        }
    }
    None
}

/// Process SQL from derive macro attribute
/// Converts the token string back to valid SQL
fn process_derive_sql(input: &str) -> String {
    // Replace escaped quotes and clean up the SQL
    input
        .replace("\\\"", "\"")
        .replace("\\n", "\n")
        .replace("\\t", "\t")
        .replace("\\\\", "\\")
}

/// Function-like macro for SQL function generation with parameter interpolation.
///
/// Transforms a function definition into a SQL query builder that:
/// - Validates SQL syntax at compile time
/// - Interpolates function parameters into {param} placeholders
/// - Returns a Sql type with the interpolated query
///
/// The function body should contain bare SQL (not quoted), with {param_name}
/// placeholders that will be replaced with the function parameters at runtime.
///
/// # Examples
///
/// ```rust,ignore
/// sql_fn! {
///     fn get_user(id: u64) -> Sql {
///         SELECT * FROM users WHERE id = {id}
///     }
/// }
///
/// let query = get_user(42);
/// ```
#[proc_macro]
pub fn sql_fn(item: TokenStream) -> TokenStream {
    // Convert to proc_macro2 for easier manipulation
    let item2: proc_macro2::TokenStream = item.into();
    let item_str = item2.to_string();

    // Parse the function signature manually from the token string
    // Format: [pub] fn name(params) -> RetType { body }
    // May have doc comments (///) and attributes (#[...]) before fn

    // Extract doc comments and attributes from the start
    let mut cleaned = item_str.trim().to_string();
    let mut doc_comments: Vec<String> = Vec::new();

    // Extract doc comments (lines starting with ///)
    loop {
        let trimmed = cleaned.trim_start();
        if let Some(pos) = trimmed.find("///") {
            // Find the end of this line
            if let Some(newline) = trimmed[pos..].find('\n') {
                // Extract the doc comment (keep the ///)
                let doc_line = trimmed[pos..pos + newline + 1].trim().to_string();
                doc_comments.push(doc_line);
                // Remove from /// to end of line
                let before = &trimmed[..pos];
                let after = &trimmed[pos + newline + 1..];
                cleaned = format!("{}{}", before, after);
            } else {
                // Doc comment at end
                let doc_line = trimmed[pos..].trim().to_string();
                if !doc_line.is_empty() {
                    doc_comments.push(doc_line);
                }
                cleaned = trimmed[..pos].trim().to_string();
                break;
            }
        } else {
            break;
        }
    }

    // Remove outer attributes (#[...])
    loop {
        let trimmed = cleaned.trim_start();
        if trimmed.starts_with("#[") {
            // Find the matching ]
            let mut depth = 1;
            let mut pos = 2;
            for c in trimmed[2..].chars() {
                if c == '[' {
                    depth += 1;
                }
                if c == ']' {
                    depth -= 1;
                }
                pos += 1;
                if depth == 0 {
                    break;
                }
            }
            cleaned = trimmed[pos..].to_string();
        } else {
            break;
        }
    }

    // Find visibility
    let trimmed = cleaned.trim_start();
    let (vis, rest) = if trimmed.starts_with("pub ") {
        ("pub ", &trimmed[4..])
    } else {
        ("", trimmed)
    };

    // Must start with "fn "
    if !rest.trim().starts_with("fn ") {
        return quote! {
            compile_error!("sql_fn! must contain a function definition starting with 'fn'")
        }
        .into();
    }

    let rest = rest.trim()[3..].trim_start(); // Skip "fn "

    // Extract function name
    let name_end = rest
        .find(|c: char| c == '(' || c.is_whitespace())
        .unwrap_or(rest.len());
    let fn_name = &rest[..name_end];

    // Find parameter list (between parentheses)
    let params_start = match rest.find('(') {
        Some(i) => i + 1,
        None => {
            return quote! {
                compile_error!("sql_fn!: expected function parameters in parentheses")
            }
            .into();
        }
    };
    let params_end = match rest.find(')') {
        Some(i) => i,
        None => {
            return quote! {
                compile_error!("sql_fn!: unclosed function parameters")
            }
            .into();
        }
    };
    let params_str = &rest[params_start..params_end];

    // Parse return type if present (between ) and {)
    let after_params = &rest[params_end + 1..].trim_start();
    let return_type = if after_params.starts_with("->") {
        let rt_start = 2;
        let rt_end = after_params.find('{').unwrap_or(after_params.len());
        after_params[rt_start..rt_end].trim().to_string()
    } else {
        "".to_string()
    };

    // Extract function body by finding matching braces
    let body_start = match cleaned.find('{') {
        Some(i) => i,
        None => {
            return quote! {
                compile_error!("sql_fn!: expected function body in braces")
            }
            .into();
        }
    };

    // Find the matching closing brace (accounting for nested braces)
    let mut brace_depth = 1;
    let mut body_end = body_start + 1;
    for c in cleaned[body_start + 1..].chars() {
        if c == '{' {
            brace_depth += 1;
        } else if c == '}' {
            brace_depth -= 1;
            if brace_depth == 0 {
                break;
            }
        }
        body_end += 1;
    }

    if brace_depth != 0 {
        return quote! {
            compile_error!("sql_fn!: unclosed function body braces")
        }
        .into();
    }

    let body_content = &cleaned[body_start + 1..body_end];

    // Parse parameters
    let params: Vec<(String, String)> = if params_str.trim().is_empty() {
        vec![]
    } else {
        params_str
            .split(',')
            .filter_map(|param| {
                let param = param.trim();
                let colon_pos = param.find(':')?;
                let name = param[..colon_pos].trim().to_string();
                let ty = param[colon_pos + 1..].trim().to_string();
                Some((name, ty))
            })
            .collect()
    };

    // Use the raw body content as SQL (trimmed)
    // The body contains bare SQL like: SELECT * FROM users WHERE id = {id}
    let sql_str = body_content.trim().to_string();

    if sql_str.is_empty() {
        return quote! {
            compile_error!("sql_fn!: function body is empty")
        }
        .into();
    }

    // Create validation SQL with ? placeholders
    let mut validation_sql = sql_str.clone();
    for (param_name, _) in &params {
        let pattern = format!("{{{}}}", param_name);
        validation_sql = validation_sql.replace(&pattern, "?");
    }

    // Validate SQL syntax
    match sqlparser::parser::Parser::parse_sql(&AnsiDialect {}, &validation_sql) {
        Ok(_) => {}
        Err(e) => {
            let msg = format!("SQL syntax error in sql_fn!: {}", e);
            return quote! {
                compile_error!(#msg)
            }
            .into();
        }
    }

    // Generate parameter identifiers for the tuple
    let param_idents: Vec<_> = params
        .iter()
        .map(|(name, _)| syn::Ident::new(name, proc_macro2::Span::call_site()))
        .collect();

    // Build the tuple expression: (param1, param2, ...)
    let param_tuple = if param_idents.is_empty() {
        quote! { () }
    } else {
        quote! { (#(#param_idents,)*) }
    };

    // Build the tuple type for Query<(Type1, Type2, ...)>
    let param_types: Vec<syn::Type> = params
        .iter()
        .map(|(_, ty)| {
            syn::parse_str::<syn::Type>(ty).unwrap_or_else(|_| syn::parse_str("()").unwrap())
        })
        .collect();

    let query_return_type = if param_types.is_empty() {
        quote! { ::sqltmpl::Query<()> }
    } else {
        quote! { ::sqltmpl::Query<( #(#param_types,)* )> }
    };

    // Parse visibility
    let vis_tokens: syn::Visibility = syn::parse_str(vis).unwrap_or(syn::Visibility::Inherited);
    let fn_name_ident = syn::Ident::new(fn_name, proc_macro2::Span::call_site());

    // Parse parameters for the function signature
    let params_tokens: syn::punctuated::Punctuated<syn::FnArg, syn::Token![,]> =
        if params_str.trim().is_empty() {
            syn::punctuated::Punctuated::new()
        } else {
            // Wrap in a dummy function to parse
            let dummy = format!("fn _dummy({}) {{}}", params_str);
            match syn::parse_str::<syn::ItemFn>(&dummy) {
                Ok(item_fn) => item_fn.sig.inputs,
                Err(_) => syn::punctuated::Punctuated::new(),
            }
        };

    // Convert doc comments to attributes
    let doc_attrs: Vec<_> = doc_comments
        .iter()
        .map(|doc| {
            let doc_lit = proc_macro2::Literal::string(&doc[3..].trim());
            quote! { #[doc = #doc_lit] }
        })
        .collect();

    // Generate the function returning Query with ? placeholder SQL and tuple params
    quote! {
        #(#doc_attrs)*
        #vis_tokens fn #fn_name_ident(#params_tokens) -> #query_return_type {
            ::sqltmpl::Query::new(
                ::sqltmpl::Sql::new(#validation_sql),
                #param_tuple
            )
        }
    }
    .into()
}

// Old extract_sql_from_body function removed - using extract_sql_string_from_body instead
