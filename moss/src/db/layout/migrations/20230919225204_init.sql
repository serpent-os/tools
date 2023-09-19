-- Add migration script here

CREATE TABLE IF NOT EXISTS layout (
    id INTEGER PRIMARY KEY AUTOINCREMENT,  
    package_id INTEGER NOT NULL,
    uid INTEGER NOT NULL,
    gid INTEGER NOT NULL,
    mode INTEGER NOT NULL,
    tag INTEGER NOT NULL,
    entry_type TEXT NOT NULL,
    entry_value1 TEXT NULL,
    entry_value2 TEXT NULL
);
