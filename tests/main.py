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
        WHERE id = 1
    """
