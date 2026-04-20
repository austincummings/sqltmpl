# sqltmpl

A Rust SQL templating library with compile-time parsing and type-safe query building.

## Overview

`sqltmpl` provides a set of macros and types for working with SQL in Rust. It parses SQL at compile time to extract metadata (tables, columns, query type, parameter counts) and provides type-safe mechanisms for binding parameters and mapping query results to structs.

## Features

- **Compile-time SQL parsing** with the `sql!` macro
- **Automatic metadata extraction**: tables, columns, query type, parameter counts
- **Type-safe parameterized queries** via `Query<T>`
- **Query result mapping** with the `#[derive(QueryResult)]` macro
- **SQL function generation** with the `sql_fn!` macro
- **Query classification**: detect SELECT vs mutating queries (INSERT, UPDATE, DELETE, etc.)
- **Zero-cost abstractions** - all parsing happens at compile time

## Usage

### Basic SQL Parsing

Use the `sql!` macro to parse SQL at compile time and get a `Sql` type with extracted metadata:

```rust
use sqltmpl::sql;

let query = sql!(SELECT id, name FROM users WHERE active = ?);

assert!(query.is_select());
assert!(!query.is_mutating());
assert_eq!(query.tables(), &["users"]);
assert_eq!(query.columns(), &["id", "name"]);
assert_eq!(query.param_count(), 1);
assert!(query.has_params());
```

### Parameterized Queries

Create type-safe queries with parameters:

```rust
use sqltmpl::{Query, sql};

let sql = sql!(INSERT INTO users (name, email) VALUES (?, ?));
let query = Query::new(sql, ("Alice".to_string(), "alice@example.com".to_string()));

let (sql, params) = query.into_parts();
assert_eq!(sql.param_count(), 2);
```

### SQL Functions

Define query functions with the `sql_fn!` macro:

```rust
use sqltmpl::sql_fn;

sql_fn! {
    pub fn get_active_users() {
        SELECT id, name FROM users WHERE active = 1
    }
}

let query = get_active_users();
```

### Query Result Mapping

Derive `QueryResult` to map database rows to structs:

```rust
use sqltmpl::{QueryResult, SimpleRow, RowAccess};

#[derive(QueryResult)]
struct User {
    id: i64,
    name: String,
}

let row = SimpleRow::new(vec!["42".to_string(), "Alice".to_string()]);
let user = User::from_row(&row).unwrap();

assert_eq!(user.id, 42);
assert_eq!(user.name, "Alice");
```

## API Reference

### `Sql` Type

The `Sql` type wraps a SQL string with compile-time extracted metadata:

- `tables()` - Returns referenced tables
- `columns()` - Returns selected columns
- `query_type()` - Returns the query type (SELECT, INSERT, UPDATE, DELETE, CREATE, DROP, or Other)
- `is_select()` - Returns true for SELECT queries
- `is_mutating()` - Returns true for INSERT, UPDATE, DELETE, DROP queries
- `param_count()` - Returns the number of `?` placeholders
- `has_params()` - Returns true if there are any parameters

### Macros

| Macro | Description |
|-------|-------------|
| `sql!(...)` | Parses SQL at compile time and returns a `Sql` type with metadata |
| `sql_fn!` | Defines functions that return pre-built `Query` objects |
| `#[derive(QueryResult)]` | Derives mapping from database rows to struct fields |

### Traits

- `RowAccess` - Abstracts over row types for column access
- `FromSql` - Converts SQL string values to Rust types (implemented for `String`, `i64`, `i32`, `f64`, `bool`)

## License

This project is licensed under the GNU General Public License v3.0 - see the [LICENSE](LICENSE) file for details.
