const VALID_QUERY = `
    SELECT name
    FROM users
    WHERE id = 1
`;

const VALID_QUERY2 = `
    SELECT name
    FROM users
    WHERE id = ${id}
`;

let VALID_QUERY3 = 'SELECT name FROM users WHERE id = 123';

let INVALID_QUERY_MISSING_TABLE = "SELECT name FROM not_existant WHERE id = 123";
