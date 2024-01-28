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

invalid_query_missing_table = """
    SELECT name
    FROM admin
    WHERE id = 1
"""
