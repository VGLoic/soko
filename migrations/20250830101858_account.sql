-- Add migration script here
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
CREATE EXTENSION IF NOT EXISTS "moddatetime";

CREATE TABLE IF NOT EXISTS "account" (
    id              UUID        NOT NULL    PRIMARY KEY DEFAULT uuid_generate_v4 (),
    email           TEXT        NOT NULL    UNIQUE,
    password_hash   TEXT        NOT NULL,
    email_verified  BOOLEAN     NOT NULL    DEFAULT FALSE,
    created_at      TIMESTAMPTZ NOT NULL    DEFAULT CURRENT_TIMESTAMP,
    updated_at      TIMESTAMPTZ NOT NULL    DEFAULT CURRENT_TIMESTAMP
);

CREATE TRIGGER update_account_moddatetime
BEFORE UPDATE ON "account"
FOR EACH ROW
EXECUTE FUNCTION moddatetime('updated_at');

CREATE TYPE verification_code_request_status AS ENUM ('active', 'cancelled', 'confirmed');

CREATE TABLE IF NOT EXISTS "verification_code_request" (
    id              UUID                                NOT NULL    PRIMARY KEY DEFAULT uuid_generate_v4 (),
    account_id      UUID                                NOT NULL,
    cyphertext      TEXT                                NOT NULL,
    status          verification_code_request_status    NOT NULL    DEFAULT 'active',
    created_at      TIMESTAMPTZ                         NOT NULL    DEFAULT CURRENT_TIMESTAMP,
    updated_at      TIMESTAMPTZ                         NOT NULL    DEFAULT CURRENT_TIMESTAMP
);

CREATE TRIGGER update_verification_code_request_moddatetime
BEFORE UPDATE ON "verification_code_request"
FOR EACH ROW
EXECUTE FUNCTION moddatetime('updated_at');
