-- Add migration script here
CREATE TABLE IF NOT EXISTS state (
    id INTEGER PRIMARY KEY AUTOINCREMENT,  
    type TEXT NOT NULL,
    created DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    summary TEXT NULL,
    description TEXT NULL
);

CREATE TABLE IF NOT EXISTS packages (
    state_id INTEGER NOT NULL,  
    package_id TEXT NOT NULL,
    reason TEXT NULL,
    FOREIGN KEY(state_id) REFERENCES state(id)
);
