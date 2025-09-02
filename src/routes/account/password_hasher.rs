#[derive(Clone, Debug)]
pub struct PasswordHasher;

impl PasswordHasher {
    /// Hash a password and a salt using SHA256, the hash is returned as base64 encoded string
    ///
    /// # Arguments
    /// * `password` - Password to hash
    pub fn hash_password(password: &str) -> Result<String, anyhow::Error> {
        bcrypt::hash(password, 12).map_err(anyhow::Error::from)
    }
}
