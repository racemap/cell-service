-- Your SQL goes here
CREATE TABLE last_updates (
    update_type ENUM('full', 'diff') NOT NULL PRIMARY KEY,
    value DATETIME NOT NULL
);