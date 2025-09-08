use argon2::{Argon2, PasswordHasher, password_hash::Salt};
use base64::prelude::*;
use rand::prelude::*;
use rand_chacha::ChaCha20Rng;

#[derive(Clone, Debug)]
pub struct PasswordStrategy;

impl PasswordStrategy {
    /// Hash a password using the Argon2id algorithm. The returned string is a argon2-formatted hash.
    ///
    /// # Arguments
    /// * `password` - Password to hash
    pub fn hash_password(password: &str) -> Result<String, anyhow::Error> {
        if password.is_empty() {
            return Err(anyhow::anyhow!("Password must not be empty"));
        }
        let mut salt = [0u8; 16];
        let mut rng = ChaCha20Rng::from_os_rng();
        rng.fill_bytes(&mut salt);
        let base64_salt = BASE64_STANDARD_NO_PAD.encode(salt);
        let argon_salt = Salt::from_b64(&base64_salt).map_err(|e| anyhow::anyhow!("{e}"))?;
        Argon2::default()
            .hash_password(password.as_bytes(), argon_salt)
            .map_err(|e| anyhow::anyhow!("{e}"))
            .map(|v| v.to_string())
    }
}
