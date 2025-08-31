-- Add migration script here
CREATE TABLE IF NOT EXISTS "account" (
    id              UUID        NOT NULL    PRIMARY KEY,
    email           TEXT        NOT NULL    UNIQUE,
    password_hash   TEXT        NOT NULL,
    email_verified  BOOLEAN     NOT NULL    DEFAULT FALSE,
    created_at      TIMESTAMPTZ NOT NULL    DEFAULT CURRENT_TIMESTAMP,
    updated_at      TIMESTAMPTZ NOT NULL    DEFAULT CURRENT_TIMESTAMP
);
