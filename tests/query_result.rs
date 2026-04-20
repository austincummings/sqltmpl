use sqltmpl::{QueryResult, SimpleRow};

#[derive(QueryResult, Debug)]
struct User {
    id: i64,
    name: String,
}

#[derive(QueryResult, Debug)]
struct LogEntry {
    id: i64,
    message: String,
    level: String,
}

#[test]
fn query_result_columns() {
    assert_eq!(User::columns(), &["id", "name"]);
}

#[test]
fn query_result_from_row() {
    let row = SimpleRow::new(vec!["42".to_string(), "Alice".to_string()]);
    let user = User::from_row(&row).unwrap();
    assert_eq!(user.id, 42);
    assert_eq!(user.name, "Alice");
}

#[test]
fn query_result_from_row_three_fields() {
    let row = SimpleRow::new(vec![
        "1".to_string(),
        "hello world".to_string(),
        "info".to_string(),
    ]);
    let entry = LogEntry::from_row(&row).unwrap();
    assert_eq!(entry.id, 1);
    assert_eq!(entry.message, "hello world");
    assert_eq!(entry.level, "info");
}

#[test]
fn query_result_from_row_out_of_bounds_fails() {
    let row = SimpleRow::new(vec!["42".to_string()]); // only 1 value, User needs 2
    assert!(User::from_row(&row).is_err());
}

#[test]
fn query_result_multiple_structs_have_independent_columns() {
    assert_eq!(User::columns(), &["id", "name"]);
    assert_eq!(LogEntry::columns(), &["id", "message", "level"]);
}
