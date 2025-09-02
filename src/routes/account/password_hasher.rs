#[derive(Clone, Debug)]
pub struct PasswordHasher;

impl PasswordHasher {
    /// Hash a password using the bcrypt algorithm. The returned string is a bcrypt-formatted hash.
    ///
    /// # Arguments
    /// * `password` - Password to hash
    pub fn hash_password(password: &str) -> Result<String, anyhow::Error> {
        bcrypt::hash(password, 12).map_err(anyhow::Error::from)
    }
}
