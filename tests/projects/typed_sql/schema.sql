-- The database this program works on (created by migrations elsewhere).
CREATE TABLE accounts (
    id INTEGER PRIMARY KEY,
    owner TEXT NOT NULL,
    balance INTEGER NOT NULL DEFAULT 0,
    note TEXT
);
CREATE INDEX accounts_owner ON accounts (owner);
