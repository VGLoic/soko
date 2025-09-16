-- Add migration script here
CREATE TABLE IF NOT EXISTS "access_token" (
    id              UUID            NOT NULL    PRIMARY KEY DEFAULT uuid_generate_v4 (),
    account_id      UUID            NOT NULL,
    name            VARCHAR(255)    NOT NULL,
    mac             bytea           NOT NULL    CHECK (length(mac) = 32),
    created_at      TIMESTAMPTZ     NOT NULL    DEFAULT CURRENT_TIMESTAMP,
    updated_at      TIMESTAMPTZ     NOT NULL    DEFAULT CURRENT_TIMESTAMP,
    expires_at      TIMESTAMPTZ     NOT NULL,
    revoked_at      TIMESTAMPTZ
);

CREATE TRIGGER update_token_moddatetime
BEFORE UPDATE ON "access_token"
FOR EACH ROW
EXECUTE FUNCTION moddatetime("updated_at");
