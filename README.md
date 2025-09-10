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

The code follows some kind of hexagonal architecture as described in this [article](https://www.howtocodeit.com/articles/master-hexagonal-architecture-rust).

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
    pub verified: bool,
    // This field is automatically set at creation at the database level
    pub created_at: DateTime<Utc>,
    // This field is automatically updated at the database level
    pub updated_at: DateTime<Utc>,
}
```
- the domain defines the data transfer objects (DTO) for each action, e.g. `signup`:
    - they carry the needed informations in order to perform the action and must define the validation rules for the action. They are most of the time built using the HTTP body and possibly other retrieval sources, e.g. database calls,
    - a dedicated error type is associated to the construction of these DTOs,
    - in addition, a dedicated error type must be added for errors occurring in adapters, e.g. database repository.
    ```rust
    /// DTO of the signup action
    /// It carries the needed informations in order to perform the signup action.
    #[derive(Debug)]
    pub struct SignupRequest {
        pub email: String,
        pub password_hash: String,
        pub verification_plaintext: u32,
        pub verification_cyphertext: String,
    }

    /// Errors in the construction of the [SignupRequest]
    #[derive(Error, Debug)]
    pub enum SignupRequestError {
        #[error("A verified account already exist for the email: {email}")]
        AccountAlreadyVerified { email: String },
        #[error(transparent)]
        Unknown(#[from] anyhow::Error),
    }

    impl SignupRequest {
        /// Build a [SignupRequest] using a [SignupBody] HTTP body
        pub fn try_from_body(body: SignupBody) -> Result<Self, SignupRequestError> {
            let password_hash = PasswordStrategy::hash_password(&body.password)?;
            let (verification_plaintext, verification_cyphertext) =
                VerificationCodeStrategy::generate_verification_code(&body.email)?;
            Ok(Self {
                email: body.email,
                password_hash,
                verification_plaintext,
                verification_cyphertext,
            })
        }

        /// Build a [SignupRequest] using a [SignupBody] HTTP body and a previously signed up account
        pub fn try_from_body_with_existing_account(
            account: Account,
            body: SignupBody,
        ) -> Result<Self, SignupRequestError> {
            if account.verified {
                return Err(SignupRequestError::AccountAlreadyVerified {
                    email: account.email,
                });
            }
            Self::try_from_body(body)
        }
    }

    /// Errors in the interactions with adapters, e.g. database repository
    #[derive(Error, Debug)]
    pub enum SignupError {
        #[error(transparent)]
        Unknown(#[from] anyhow::Error),
    }
    ```
- if needed, the domain must define a `repository` in order to abstract the database interactions related to the domain entities. This repository must be exposed as a documented `trait`. Errors must be the ones defined by the domain
```rust
#[async_trait]
pub trait AccountRepository: Send + Sync {
    /// Create an account and creates an active verification request
    ///
    /// # Arguments
    /// * `email` - Email of the account,
    /// * `password_hash` - Hash of the password,
    /// * `verification_cyphertext` - Cyphertext of the verification request
    ///
    /// # Errors
    /// * `SignupError::Unknown` - unknown error type
    async fn create_account(&self, signup_request: &SignupRequest) -> Result<Account, SignupError>;
}
```
- each route handler must be defined in a dedicated handler function. A handler function returns a result of the form `Result<(StatusCode, Json<ResponseType>), ApiError>`,
    ```rust
    async fn signup_account(
        State(app_state): State<AppState>,
        ValidatedJson(body): ValidatedJson<SignupBody>,
    ) -> Result<(StatusCode, Json<AccountResponse>), ApiError> {
        ...
    ```
- the domain API errors must be defined as an enum that implements the `IntoResponse` trait of `axum`
    ```rust
    #[derive(Debug)]
    pub enum ApiError {
        InternalServerError(anyhow::Error),
        BadRequest(ValidationErrors),
        NotFound,
    }

    impl IntoResponse for ApiError {
        fn into_response(self) -> Response {
            match self {
                Self::InternalServerError(e) => {
                    error!("{e}");
                    (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error").into_response()
                }
                Self::BadRequest(errors) => (StatusCode::BAD_REQUEST, Json(errors)).into_response(),
                Self::NotFound => (StatusCode::NOT_FOUND, "Not found").into_response(),
            }
        }
    }
    ```
- the mapping from the previously defined domain errors and the domain API errors must be defined,
    ```rust
    impl From<SignupError> for ApiError {
        fn from(value: SignupError) -> Self {
            match value {
                SignupError::Unknown(e) => ApiError::InternalServerError(e),
            }
        }
    }

    impl From<SignupRequestError> for ApiError {
        fn from(value: SignupRequestError) -> ApiError {
            match value {
                SignupRequestError::Unknown(e) => ApiError::InternalServerError(e),
                SignupRequestError::AccountAlreadyVerified { email: _email } => {
                    let mut errors = ValidationErrors::new();
                    errors.add(
                        "email",
                        ValidationError::new("existing-email")
                            .with_message("Email is already associated with a verified account".into()),
                    );
                    ApiError::BadRequest(errors)
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
