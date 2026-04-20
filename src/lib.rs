//! SQL string type with compile-time validation via proc-macro
//!
//! This crate provides:
//! - `Sql` type: A wrapper around SQL strings with trait implementations
//! - `sql!` macro: Compile-time SQL syntax validation
//! - `#[derive(QueryResult)]`: Type-safe struct generation from SQL
//! - `RowAccess` trait: Database integration for struct mapping
//!
//! # Examples
//!
//! ```
//! use sqltmpl::{Sql, sql};
//!
//! // Create SQL via the macro (validated at compile time)
//! let query: Sql = sql!(SELECT * FROM users WHERE id = 1);
//!
//! // Use with database libraries
//! // conn.execute(&query, [])?;
//! ```

pub use sqltmpl_macros::{QueryResult, sql, sql_fn};

/// A validated SQL string type.
///
/// This type wraps a string containing SQL. It is typically created via the `sql!` macro
/// which validates the SQL syntax at compile time.
///
/// # Examples
///
/// ```ignore
/// use sqltmpl::{Sql, sql};
///
/// // Via macro (validated at compile time)
/// let query: Sql = sql!(SELECT * FROM users WHERE id = 1);
///
/// // Via constructor (runtime, unchecked)
/// let query = Sql::new("SELECT * FROM users");
/// ```
/// The type of SQL query
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum QueryType {
    Select,
    Insert,
    Update,
    Delete,
    Create,
    Drop,
    Other,
}

/// A validated SQL string with compile-time introspection metadata
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Sql {
    value: &'static str,
    tables: &'static [&'static str],
    columns: &'static [&'static str],
    query_type: QueryType,
    param_count: usize,
}

impl Sql {
    /// Create a new Sql instance from a string.
    ///
    /// Note: This does not validate the SQL. For compile-time validation,
    /// use the `sql!` macro instead.
    pub fn new<S: AsRef<str>>(sql: S) -> Self {
        Self {
            value: Box::leak(sql.as_ref().to_string().into_boxed_str()),
            tables: &[],
            columns: &[],
            query_type: QueryType::Other,
            param_count: 0,
        }
    }

    /// Create a new Sql instance with full metadata.
    ///
    /// This is used internally by the `sql!` macro.
    pub const fn new_with_metadata(
        sql: &'static str,
        tables: &'static [&'static str],
        columns: &'static [&'static str],
        query_type: QueryType,
        param_count: usize,
    ) -> Self {
        Self {
            value: sql,
            tables,
            columns,
            query_type,
            param_count,
        }
    }

    /// Create a new Sql instance from a static string (basic).
    ///
    /// This is used internally by the `sql!` macro for simple cases.
    pub const fn new_const(sql: &'static str) -> Self {
        Self {
            value: sql,
            tables: &[],
            columns: &[],
            query_type: QueryType::Other,
            param_count: 0,
        }
    }

    /// Get a string slice of the SQL.
    pub const fn as_str(&self) -> &str {
        self.value
    }

    /// Convert into a String.
    pub fn to_string(&self) -> String {
        self.value.to_string()
    }

    /// Get the underlying &'static str.
    pub const fn into_inner(self) -> &'static str {
        self.value
    }

    // ==================== INTROSPECTION METHODS ====================

    /// Get the list of tables referenced in the query.
    pub const fn tables(&self) -> &'static [&'static str] {
        self.tables
    }

    /// Get the list of columns selected/returned (for SELECT queries).
    pub const fn columns(&self) -> &'static [&'static str] {
        self.columns
    }

    /// Get the query type (SELECT, INSERT, UPDATE, etc.).
    pub const fn query_type(&self) -> QueryType {
        self.query_type
    }

    /// Check if this is a SELECT query.
    pub const fn is_select(&self) -> bool {
        matches!(self.query_type, QueryType::Select)
    }

    /// Check if this query modifies data (INSERT, UPDATE, DELETE).
    pub const fn is_mutating(&self) -> bool {
        matches!(
            self.query_type,
            QueryType::Insert | QueryType::Update | QueryType::Delete
        )
    }

    /// Get the number of parameters (placeholders like ?).
    pub const fn param_count(&self) -> usize {
        self.param_count
    }

    /// Check if the query has any parameters.
    pub const fn has_params(&self) -> bool {
        self.param_count > 0
    }
}

impl AsRef<str> for Sql {
    fn as_ref(&self) -> &str {
        &self.value
    }
}

impl AsRef<[u8]> for Sql {
    fn as_ref(&self) -> &[u8] {
        self.value.as_bytes()
    }
}

impl std::ops::Deref for Sql {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl From<Sql> for String {
    fn from(sql: Sql) -> Self {
        sql.value.to_string()
    }
}

impl From<&Sql> for String {
    fn from(sql: &Sql) -> Self {
        sql.value.to_string()
    }
}

impl From<Sql> for std::borrow::Cow<'static, str> {
    fn from(sql: Sql) -> Self {
        std::borrow::Cow::Borrowed(sql.value)
    }
}

impl std::fmt::Display for Sql {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.value)
    }
}

impl Default for Sql {
    fn default() -> Self {
        Self {
            value: "",
            tables: &[],
            columns: &[],
            query_type: QueryType::Other,
            param_count: 0,
        }
    }
}

// Allow Sql to be used directly in duckdb and other database operations
// that expect &str via deref coercion
impl From<Sql> for &'static str {
    fn from(sql: Sql) -> Self {
        sql.value
    }
}

// Allow comparison with &str
impl PartialEq<&str> for Sql {
    fn eq(&self, other: &&str) -> bool {
        self.value == *other
    }
}

impl PartialEq<str> for Sql {
    fn eq(&self, other: &str) -> bool {
        self.value == other
    }
}

impl PartialEq<String> for Sql {
    fn eq(&self, other: &String) -> bool {
        self.value == other.as_str()
    }
}

/// Trait for accessing row data by index.
/// Implemented by database row types to allow `QueryResult` generated
/// structs to be created from query results.
pub trait RowAccess {
    /// Get the number of columns in the row
    fn len(&self) -> usize;

    /// Check if the row is empty
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Get a value at the specified index, converting to type T
    fn get<T: FromSql>(&self, index: usize) -> Result<T, Box<dyn std::error::Error>>;
}

/// Trait for converting SQL values to Rust types.
/// Implement this for types you want to extract from rows.
pub trait FromSql: Sized {
    fn from_sql(value: &str) -> Result<Self, Box<dyn std::error::Error>>;
}

// Implementations for common types
impl FromSql for String {
    fn from_sql(value: &str) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(value.to_string())
    }
}

impl FromSql for i64 {
    fn from_sql(value: &str) -> Result<Self, Box<dyn std::error::Error>> {
        value
            .parse()
            .map_err(|e| format!("Failed to parse i64: {}", e).into())
    }
}

impl FromSql for i32 {
    fn from_sql(value: &str) -> Result<Self, Box<dyn std::error::Error>> {
        value
            .parse()
            .map_err(|e| format!("Failed to parse i32: {}", e).into())
    }
}

impl FromSql for f64 {
    fn from_sql(value: &str) -> Result<Self, Box<dyn std::error::Error>> {
        value
            .parse()
            .map_err(|e| format!("Failed to parse f64: {}", e).into())
    }
}

impl FromSql for bool {
    fn from_sql(value: &str) -> Result<Self, Box<dyn std::error::Error>> {
        match value.to_lowercase().as_str() {
            "true" | "1" | "yes" | "t" => Ok(true),
            "false" | "0" | "no" | "f" => Ok(false),
            _ => Err(format!("Failed to parse bool from: {}", value).into()),
        }
    }
}

/// A simple in-memory row implementation for testing and examples
#[derive(Debug, Clone)]
pub struct SimpleRow {
    values: Vec<String>,
}

impl SimpleRow {
    pub fn new(values: Vec<String>) -> Self {
        Self { values }
    }
}

impl RowAccess for SimpleRow {
    fn len(&self) -> usize {
        self.values.len()
    }

    fn get<T: FromSql>(&self, index: usize) -> Result<T, Box<dyn std::error::Error>> {
        if index >= self.values.len() {
            return Err(
                format!("Index {} out of bounds (len: {})", index, self.values.len()).into(),
            );
        }
        T::from_sql(&self.values[index])
    }
}

/// A parameterized SQL query with type-safe tuple parameters.
///
/// This struct stores SQL with `?` placeholders and the parameter values
/// separately in a tuple. This prevents SQL injection while maintaining
/// type safety and database agnosticism.
///
/// # Type Parameters
///
/// * `T` - A tuple type containing the query parameters (e.g., `(u64,)` or `(&str, i32)`)
///
/// # Examples
///
/// ```
/// use sqltmpl::{Sql, Query};
///
/// // Create a query manually
/// let query = Query::new(
///     Sql::new("SELECT * FROM users WHERE id = ?"),
///     (42i64,),
/// );
///
/// // Access the SQL and parameters
/// assert_eq!(query.sql().as_str(), "SELECT * FROM users WHERE id = ?");
/// assert_eq!(query.params().0, 42i64);
/// ```
pub struct Query<T> {
    sql: Sql,
    params: T,
}

impl<T> Query<T> {
    /// Create a new Query with SQL and parameters.
    pub fn new(sql: Sql, params: T) -> Self {
        Self { sql, params }
    }

    /// Get a reference to the SQL.
    pub fn sql(&self) -> &Sql {
        &self.sql
    }

    /// Get a reference to the parameters tuple.
    pub fn params(&self) -> &T {
        &self.params
    }

    /// Consume the query and return the SQL and parameters.
    pub fn into_parts(self) -> (Sql, T) {
        (self.sql, self.params)
    }
}

impl<T: std::fmt::Debug> std::fmt::Debug for Query<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Query")
            .field("sql", &self.sql)
            .field("params", &self.params)
            .finish()
    }
}

impl<T: Clone> Clone for Query<T> {
    fn clone(&self) -> Self {
        Self {
            sql: self.sql.clone(),
            params: self.params.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sql_new() {
        let sql = Sql::new("SELECT * FROM users");
        assert_eq!(sql.as_str(), "SELECT * FROM users");
    }

    #[test]
    fn test_sql_as_ref() {
        let sql = Sql::new("SELECT 1");
        let s: &str = sql.as_ref();
        assert_eq!(s, "SELECT 1");
    }

    #[test]
    fn test_sql_deref() {
        let sql = Sql::new("SELECT 1");
        assert_eq!(sql.len(), 8);
        assert_eq!(&*sql, "SELECT 1");
    }

    #[test]
    fn test_sql_into_string() {
        let sql = Sql::new("SELECT 1");
        let s: String = sql.to_string();
        assert_eq!(s, "SELECT 1");
    }

    #[test]
    fn test_sql_display() {
        let sql = Sql::new("SELECT 1");
        assert_eq!(format!("{}", sql), "SELECT 1");
    }

    #[test]
    fn test_sql_equality() {
        let sql = Sql::new("SELECT 1");
        assert_eq!(sql, "SELECT 1");
        assert_eq!(sql, String::from("SELECT 1"));
    }

    #[test]
    fn test_sql_introspection() {
        // Create SQL with metadata
        let sql = Sql::new_with_metadata(
            "SELECT id, name FROM users WHERE active = ?",
            &["users"],
            &["id", "name"],
            QueryType::Select,
            1,
        );

        assert_eq!(sql.as_str(), "SELECT id, name FROM users WHERE active = ?");
        assert_eq!(sql.tables(), &["users"]);
        assert_eq!(sql.columns(), &["id", "name"]);
        assert!(sql.is_select());
        assert!(!sql.is_mutating());
        assert_eq!(sql.param_count(), 1);
        assert!(sql.has_params());
    }

    #[test]
    fn test_sql_insert_metadata() {
        let sql = Sql::new_with_metadata(
            "INSERT INTO logs VALUES (?, ?, ?)",
            &["logs"],
            &[],
            QueryType::Insert,
            3,
        );

        assert!(!sql.is_select());
        assert!(sql.is_mutating());
        assert_eq!(sql.tables(), &["logs"]);
        assert_eq!(sql.param_count(), 3);
    }

    #[test]
    fn test_simple_row() {
        let row = SimpleRow::new(vec![
            "42".to_string(),
            "Alice".to_string(),
            "true".to_string(),
        ]);

        assert_eq!(row.len(), 3);
        assert_eq!(row.get::<i64>(0).unwrap(), 42);
        assert_eq!(row.get::<String>(1).unwrap(), "Alice");
        assert_eq!(row.get::<bool>(2).unwrap(), true);
    }

    #[test]
    fn test_sql_default() {
        let sql = Sql::default();
        assert_eq!(sql.as_str(), "");
        assert!(!sql.has_params());
        assert!(!sql.is_select());
        assert!(!sql.is_mutating());
    }

    #[test]
    fn test_sql_clone() {
        let sql = Sql::new("SELECT 1");
        assert_eq!(sql.clone(), sql);
    }

    #[test]
    fn test_sql_new_const() {
        const SQL: Sql = Sql::new_const("SELECT 1");
        assert_eq!(SQL.as_str(), "SELECT 1");
    }

    #[test]
    fn test_sql_into_inner() {
        let sql = Sql::new_const("SELECT 1");
        let s: &'static str = sql.into_inner();
        assert_eq!(s, "SELECT 1");
    }

    #[test]
    fn test_sql_into_cow() {
        let sql = Sql::new_const("SELECT 1");
        let cow: std::borrow::Cow<'static, str> = sql.into();
        assert_eq!(cow, "SELECT 1");
    }

    #[test]
    fn test_sql_into_static_str() {
        let sql = Sql::new_const("SELECT 1");
        let s: &'static str = sql.into();
        assert_eq!(s, "SELECT 1");
    }

    #[test]
    fn test_sql_has_params_false() {
        let sql = Sql::new("SELECT * FROM users");
        assert!(!sql.has_params());
        assert_eq!(sql.param_count(), 0);
    }

    #[test]
    fn test_sql_update_is_mutating() {
        let sql = Sql::new_with_metadata(
            "UPDATE users SET name = ? WHERE id = ?",
            &["users"],
            &[],
            QueryType::Update,
            2,
        );
        assert!(sql.is_mutating());
        assert!(!sql.is_select());
    }

    #[test]
    fn test_sql_delete_is_mutating() {
        let sql = Sql::new_with_metadata(
            "DELETE FROM users WHERE id = ?",
            &["users"],
            &[],
            QueryType::Delete,
            1,
        );
        assert!(sql.is_mutating());
        assert!(!sql.is_select());
    }

    #[test]
    fn test_simple_row_is_empty() {
        assert!(SimpleRow::new(vec![]).is_empty());
        assert!(!SimpleRow::new(vec!["x".to_string()]).is_empty());
    }

    #[test]
    fn test_simple_row_out_of_bounds() {
        let row = SimpleRow::new(vec!["42".to_string()]);
        assert!(row.get::<i64>(1).is_err());
    }

    #[test]
    fn test_from_sql_i32() {
        assert_eq!(i32::from_sql("99").unwrap(), 99i32);
        assert!(i32::from_sql("not_a_number").is_err());
    }

    #[test]
    fn test_from_sql_f64() {
        assert_eq!(f64::from_sql("3.14").unwrap(), 3.14f64);
        assert!(f64::from_sql("not_a_float").is_err());
    }

    #[test]
    fn test_from_sql_bool_variants() {
        for s in ["true", "1", "yes", "t"] {
            assert!(bool::from_sql(s).unwrap(), "expected true for {s}");
        }
        for s in ["false", "0", "no", "f"] {
            assert!(!bool::from_sql(s).unwrap(), "expected false for {s}");
        }
        assert!(bool::from_sql("maybe").is_err());
    }

    #[test]
    fn test_query_accessors() {
        let query = Query::new(
            Sql::new("SELECT * FROM users WHERE id = ?"),
            (42i64,),
        );
        assert_eq!(query.sql().as_str(), "SELECT * FROM users WHERE id = ?");
        assert_eq!(query.params().0, 42i64);
    }

    #[test]
    fn test_query_into_parts() {
        let (sql, params) = Query::new(
            Sql::new("SELECT * FROM t WHERE id = ?"),
            (7i64,),
        ).into_parts();
        assert_eq!(sql.as_str(), "SELECT * FROM t WHERE id = ?");
        assert_eq!(params.0, 7i64);
    }

    // Property-based tests using proptest
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn query_tuple_matches_params(i in 0i64..1000i64) {
            // Property: Query with single param should have matching SQL placeholder count
            let query = Query::new(
                Sql::new("SELECT * FROM users WHERE id = ?"),
                (i,),
            );

            // The SQL should have exactly one ? placeholder
            assert_eq!(query.sql().as_str().matches('?').count(), 1);

            // The tuple should have exactly one element
            let (param,) = query.params();
            assert_eq!(*param, i);
        }

        #[test]
        fn query_clone_preserves_params(i in 0i64..1000i64, j in 0i64..1000i64) {
            // Property: Cloning a Query preserves SQL and params
            let query = Query::new(
                Sql::new("SELECT * FROM t WHERE a = ? AND b = ?"),
                (i, j),
            );

            let cloned = query.clone();

            assert_eq!(cloned.sql().as_str(), query.sql().as_str());
            assert_eq!(cloned.params().0, query.params().0);
            assert_eq!(cloned.params().1, query.params().1);
        }

        #[test]
        fn sql_roundtrip_parsing(sql in "SELECT [a-z]+ FROM [a-z]+( WHERE [a-z]+ = \\?)?") {
            // Property: Valid SQL patterns should parse successfully
            // This uses sqlparser to validate the SQL syntax
            use sqlparser::dialect::AnsiDialect;
            use sqlparser::parser::Parser;

            let result = Parser::parse_sql(&AnsiDialect {}, &sql);
            prop_assert!(result.is_ok(), "SQL should parse: {}", sql);
        }

        #[test]
        fn query_into_parts_roundtrip(i in 0i64..1000i64) {
            // Property: into_parts returns the original SQL and params
            let query = Query::new(
                Sql::new("SELECT * FROM users WHERE id = ?"),
                (i,),
            );

            let (sql, params) = query.into_parts();

            assert_eq!(sql.as_str(), "SELECT * FROM users WHERE id = ?");
            assert_eq!(params.0, i);
        }
    }
}
