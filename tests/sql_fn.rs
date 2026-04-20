use sqltmpl::{Query, sql_fn};

sql_fn! {
    fn all_users() {
        SELECT * FROM users
    }
}

sql_fn! {
    pub fn active_sessions() {
        SELECT id, user_id FROM sessions WHERE active = 1
    }
}

#[test]
fn sql_fn_no_params_returns_query() {
    // sql_fn! uses Sql::new (no metadata), so query_type is Other
    let q: Query<()> = all_users();
    assert_eq!(q.sql().param_count(), 0);
    assert!(q.sql().as_str().contains("users"), "SQL: {}", q.sql().as_str());
}

#[test]
fn sql_fn_no_params_sql_content() {
    let q = all_users();
    assert!(q.sql().as_str().contains("users"), "expected 'users' in: {}", q.sql().as_str());
}

#[test]
fn sql_fn_pub_visibility_is_callable() {
    let q = active_sessions();
    assert!(q.sql().as_str().contains("sessions"), "SQL: {}", q.sql().as_str());
}

#[test]
fn sql_fn_no_params_empty_tuple() {
    let q = all_users();
    let (_, params) = q.into_parts();
    let () = params;
}
