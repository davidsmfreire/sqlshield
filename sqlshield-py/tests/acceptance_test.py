from sqlshield import validate_query

def test_validate_query_from_python():
    schema = """
        CREATE TABLE users (
            id INT PRIMARY KEY AUTO_INCREMENT,
            name VARCHAR(255) NOT NULL
        );
    """

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

    assert validate_query(valid_query, schema) == []
    assert validate_query(invalid_query_missing_column, schema) == ['Column `email` not found in table `users`']
