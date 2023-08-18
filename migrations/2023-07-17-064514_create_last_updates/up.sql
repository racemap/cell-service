-- Your SQL goes here
CREATE TABLE last_updates (
    value DATETIME NOT NULL PRIMARY KEY,
    update_type ENUM('full', 'diff') NOT NULL
);