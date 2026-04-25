CREATE TABLE users (
    id INT PRIMARY KEY AUTO_INCREMENT,
    name VARCHAR(255) NOT NULL
);


CREATE TABLE receipt (
    id INT PRIMARY KEY AUTO_INCREMENT,
    content VARCHAR(128),
    user_id INT,
    FOREIGN KEY (user_id) REFERENCES users(id)
);
