valid_query = """
    SELECT name
    FROM users
    WHERE id = 1
"""

invalid_query_missing_column = """
    SELECT email
    FROM users
    WHERE id = 1
"""

INVALID_QUERY_MISSING_TABLE = """
    SELECT name
    FROM admin
    WHERE id = 1
"""

class Repository:
    def fn():
        invalid_query_missing_table_in_fn = """
            SELECT name
            FROM admin
            WHERE id = 1
        """

def fn():
    invalid_query_missing_table_in_fn = """
        SELECT name
        FROM admin
        WHERE id = {something}
    """


valid_query_with_alias_and_join = """
    SELECT u.name, r.content
    FROM users u
    JOIN receipt r
    ON r.user_id = u.id
    WHERE r.id = 1
"""

invalid_query_with_alias_and_join = f"""
    SELECT r.name, u.content
    FROM users u
    JOIN receipt r
    ON r.user_id = u.id
    WHERE r.id = {1}
"""

valid_query_with_derived = """
    WITH sub as (
        SELECT user_id, content FROM receipt
    )
    SELECT u.id, k.content
    FROM users u
    JOIN sub k
    ON k.user_id = u.id
"""

invalid_query_with_derived_no_table = """
    WITH sub as (
        SELECT user_id, content FROM admin
    )
    SELECT k.user_id, u.id
    FROM users u
    JOIN sub k
    ON k.user_id = u.id
"""

invalid_query_with_derived_wrong_columns = """
    WITH sub as (
        SELECT user_id, content FROM receipt
    )
    SELECT k.id, u.content
    FROM users u
    JOIN sub k
    ON k.user_id = u.id
"""