use sqltmpl::{Sql, sql};

#[test]
fn sql_macro_select_star_metadata() {
    let q = sql!(SELECT * FROM users);
    assert!(q.is_select());
    assert!(!q.is_mutating());
    assert_eq!(q.tables(), &["users"]);
    assert!(q.columns().contains(&"*"));
    assert_eq!(q.param_count(), 0);
    assert!(!q.has_params());
}

#[test]
fn sql_macro_select_columns_metadata() {
    let q = sql!(SELECT id, name FROM users WHERE active = ?);
    assert!(q.is_select());
    assert_eq!(q.tables(), &["users"]);
    assert_eq!(q.columns(), &["id", "name"]);
    assert_eq!(q.param_count(), 1);
    assert!(q.has_params());
}

#[test]
fn sql_macro_insert_metadata() {
    let q = sql!(INSERT INTO logs (msg, level) VALUES (?, ?));
    assert!(q.is_mutating());
    assert!(!q.is_select());
    assert_eq!(q.tables(), &["logs"]);
    assert_eq!(q.param_count(), 2);
}

#[test]
fn sql_macro_update_metadata() {
    let q = sql!(UPDATE users SET name = ? WHERE id = ?);
    assert!(q.is_mutating());
    assert!(!q.is_select());
    assert_eq!(q.param_count(), 2);
}

#[test]
fn sql_macro_delete_metadata() {
    let q = sql!(DELETE FROM sessions WHERE expires_at < ?);
    assert!(q.is_mutating());
    assert!(!q.is_select());
    assert_eq!(q.param_count(), 1);
}

#[test]
fn sql_macro_produces_sql_type() {
    let q: Sql = sql!(SELECT 1);
    assert!(q.is_select());
    assert_eq!(q.param_count(), 0);
}

#[test]
fn sql_macro_multiple_params() {
    let q = sql!(SELECT * FROM t WHERE a = ? AND b = ? AND c = ?);
    assert_eq!(q.param_count(), 3);
}

#[test]
fn sql_macro_join() {
    let q = sql!(SELECT u.id FROM users u JOIN orders o ON u.id = o.user_id);
    assert!(q.is_select());
    assert!(q.tables().contains(&"users") || q.tables().contains(&"u"));
}
