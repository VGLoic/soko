-- Add migration script here
CREATE TABLE IF NOT EXISTS "account" (
    id          UUID        NOT NULL    PRIMARY KEY,
    email       TEXT        NOT NULL    UNIQUE,
    created_at  TIMESTAMPTZ NOT NULL    DEFAULT CURRENT_TIMESTAMP,
    updated_at  TIMESTAMPTZ NOT NULL    DEFAULT CURRENT_TIMESTAMP
);
