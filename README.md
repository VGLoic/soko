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

### Integration tests

Integration tests require a database running and exposed on port 5433, use the related docker compose for it:
```bash
docker compose -f compose.integration.yaml up
```

Once the database is up, integration tests can be run:
```bash
cargo test --tests
```

Alternatively, a script has been added in order to wrap the tests with the database container mounting and unmounting:
```bash
# Allow the script to run
chmod +x scripts/integration-test.sh
./scripts/integration-test.sh
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

### Domain implementation

The routes are split in various business domains. A domain is meant to be contained as much as possible.

Implementation of a domain must follow a set of rules:
- the domain must expose its own router factory, exposing its routes,
    ```rust
    pub fn account_router() -> Router<AppState> {
        Router::new().route("/signup", post(signup_account))
    }
    ```
- the domain defines one or multiple entities in order to define it. These entities are implemented as Rust structures,
    ```rust
    #[derive(FromRow)]
    pub struct Account {
        pub id: uuid::Uuid,
        pub email: String,
        pub password_hash: String,
        pub email_verified: bool,
        pub created_at: DateTime<Utc>,
        pub updated_at: DateTime<Utc>,
    }
    ```
- each domain entity must implement its set of methods in order to define what business rules are allowed for this entity. We define this collection of methods the `entity model`,
    ```rust
    impl Account {
        /// Update the password hash of an account
        ///
        /// # Arguments
        /// * `password_hash` - Updated password hash
        pub fn update_password_hash(&mut self, password_hash: String) -> &mut Self {
            self.password_hash = password_hash;
            self.updated_at = Utc::now();
            self
        }
    }
    ```
- if needed, the domain must define a `repository` in order to abstract the database interactions related to the domain entities. This repository must be exposed as a documented `trait` and have its own errors defined as an `enum`,
    ```rust
    #[async_trait]
    pub trait AccountRepository: Send + Sync {
        /// Get an account by email
        ///
        /// # Arguments
        /// * `email` - Email of the account
        ///
        /// # Errors
        /// - `Unclassified`: fallback error type
        async fn get_account_by_email(
            &self,
            email: &str,
        ) -> Result<Option<Account>, AccountRepositoryError>;

        /// Update an account identified by its ID
        ///
        /// # Arguments
        /// * `account` - Updated account,
        ///
        /// # Errors
        /// - `AccountNotFound`: account not found
        /// - `Unclassified`: fallback error type
        async fn update_account(&self, account: &Account) -> Result<(), AccountRepositoryError>;

        /// Crate an account
        ///
        /// # Arguments
        /// * `email` - Email of the account,
        /// * `password_hash` - Hash of the password
        ///
        /// # Errors
        /// - `AccountNotFound`: account not found after creation
        /// - `Unclassified`: fallback error type
        async fn create_account(
            &self,
            email: &str,
            password_hash: &str,
        ) -> Result<Account, AccountRepositoryError>;
    }

    #[derive(Error, Debug)]
    pub enum AccountRepositoryError {
        #[error(transparent)]
        Unclassified(#[from] anyhow::Error),
        #[error("Account not found using search param: {0}")]
        AccountNotFound(String),
    }
    ```
- each route handler must be defined in a dedicated handler function. A handler function returns a result of the form `Result<(StatusCode, Json<ResponseType>), DomainRouteError>`,
    ```rust
    async fn signup_account(
        State(app_state): State<AppState>,
        ValidatedJson(payload): ValidatedJson<SignupPayload>,
    ) -> Result<(StatusCode, Json<AccountResponse>), AccountError> {
        ...
    ```
- the domain route errors must be defined as an enum that implements the `IntoResponse` trait of `axum`,
    ```rust
    #[derive(Error, Debug)]
    pub enum AccountError {
        #[error(transparent)]
        Unclassified(#[from] anyhow::Error),
        #[error("A verified account already exist for the email: {0}")]
        AccountAlreadyVerified(String),
    }

    impl IntoResponse for AccountError {
        fn into_response(self) -> axum::response::Response {
            error!("{self}");
            match self {
                Self::Unclassified(_) => {
                    (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error").into_response()
                }
                Self::AccountAlreadyVerified(_) => {
                    let mut errors = ValidationErrors::new();
                    errors.add(
                        "email",
                        ValidationError::new("existing-email")
                            .with_message("Email is already used for another account".into()),
                    );
                    (StatusCode::BAD_REQUEST, Json(errors)).into_response()
                }
            }
        }
    }
    ```
- the response type must be serializable in JSON format, the handlers must try to have response type as close as possible if applicable
    ```rust
    #[derive(Debug, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct AccountResponse {
        pub email: String,
        pub created_at: DateTime<Utc>,
        pub updated_at: DateTime<Utc>,
    }

    impl From<model::Account> for AccountResponse {
        fn from(value: model::Account) -> Self {
            AccountResponse {
                email: value.email,
                created_at: value.created_at,
                updated_at: value.updated_at,
            }
        }
    }
    ```
