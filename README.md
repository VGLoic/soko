# Soko

Soko is a service that helps you manage your Ethereum smart contracts releases. It provides a simple interface for pushing and pulling compilation artifacts between your local environment and the Soko cloud.

## Domains

### Account

It represents a user account in the Soko system.

The related actions are:
- **sign up**: allows a user to create a new unverified account with a mail and a password,
- **confirm sign up**: allows a user to confirm their email address and complete the sign-up process,
- **generate an access token**: allows a user to generate a new short lived access token for their account.

All the actions are authenticated using the email and password couple.

### Project

It represents a project in the Soko system, it contains a collection of smart contracts compilation artifacts. It is owned by a user account.

The related actions are:
- **create**: allows a user to create a new project,
- **rename**: allows a user to rename an existing project,
- **delete**: allows a user to delete a project,
- **list compilation artifacts**: allows a user to list all artifacts for a project.

All the actions are authenticated using an access token.

### Artifact

It represents a compilation artifact in the Soko system. An artifact belongs to a project. It is identified by a unique ID. It can also be tagged with an arbitrary label.

The related actions are:
- **push**: allows a user to push a new artifact for a project,
- **pull**: allows a user to pull an existing artifact for a project,
- **describe**: allows a user to get information about a specific artifact.

## Local development

To get started with local development, you'll need to set up your environment. Follow these steps:

1. Make sure you have [Rust](https://www.rust-lang.org/tools/install) installed on your machine. Cargo version at the time of writing is 1.88.0.
    ```bash
    cargo --version
    ```

2. Set up the environment variables in a `.env` file, the required ones are indicated with the `REQUIRED` label.
    ```bash
    cp .env.example .env
    ```

3. Verify that the unit tests are running:
    ```bash
    cargo test
    ```

4. Verify that the integration tests are running:
    ```bash
    docker compose -f compose.integration.yaml up
    cargo test --test integration
    # Once the integration tests have finished
    docker compose -f compose.integration.yaml down -v
    ```

5. Launch the database locally:
    ```bash
    docker compose up
    ```

7. Run the application
    ```bash
    cargo run .
    ```

### Database interaction and migration

Soko uses [`sqlx`](https://github.com/launchbadge/sqlx) for database connectivity and migrations.

#### Migration commands

- **Create a new migration (no running database required):**
    ```bash
    cargo sqlx migrate add <migration_name>
    ```

- **Run and check migrations (requires running database):**
    - Ensure your database connection is configured in `.env` (see `.env.example` for required variables, e.g. `DATABASE_URL`).
    - Run migrations:
        ```bash
        cargo sqlx migrate run
        ```
    - Check migration status:
        ```bash
        cargo sqlx migrate info
        ```
    - Revert the last migration:
        ```bash
        cargo sqlx migrate revert
        ```

#### Troubleshooting

- If you encounter connection errors, verify that your database is running and your `.env` configuration is correct.
- For more details, see the [`sqlx-cli` documentation](https://github.com/launchbadge/sqlx/blob/main/sqlx-cli/README.md) and the [`sqlx` docs](https://github.com/launchbadge/sqlx).
