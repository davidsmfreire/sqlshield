#![allow(dead_code)]
#![allow(unused_variables)]

const VALID_QUERY: &str = "
    SELECT name
    FROM users
    WHERE id = 1
";

const INVALID_QUERY_MISSING_COLUMN: &str = "
    SELECT email
    FROM users
    WHERE id = 1
";

const INVALID_QUERY_MISSING_TABLE: &str = "
    SELECT name
    FROM admin
    WHERE id = 1
";

struct Repository;

impl Repository {
    fn new() {
        let id = 1;
        let invalid_query_missing_table_in_method = format!(
            "
            SELECT name
            FROM admin
            WHERE id = {id}
        "
        );
    }
}

fn func() {
    let invalid_query_missing_table_in_fn = "
        SELECT name
        FROM admin
        WHERE id = 1
    ";
}

fn main() {
    let valid_query_with_alias_and_join = "
        SELECT u.name, r.content
        FROM users u
        JOIN receipt r
        ON r.user_id = u.id
        WHERE r.id = 1
    ";

    let invalid_query_with_alias_and_join = "
        SELECT r.name, u.content
        FROM users u
        JOIN receipt r
        ON r.user_id = u.id
        WHERE r.id = 1
    ";

    let valid_query_with_derived = "
        WITH sub as (
            SELECT user_id, content FROM receipt
        )
        SELECT u.id, k.content
        FROM users u
        JOIN sub k
        ON k.user_id = u.id
    ";

    let invalid_query_with_derived_no_table = "
        WITH sub as (
            SELECT user_id, content FROM admin
        )
        SELECT k.user_id, u.id
        FROM users u
        JOIN sub k
        ON k.user_id = u.id
    ";

    let invalid_query_with_derived_wrong_columns = "
        WITH sub as (
            SELECT user_id, content FROM receipt
        )
        SELECT k.id, u.content
        FROM users u
        JOIN sub k
        ON k.user_id = u.id
    ";
}
