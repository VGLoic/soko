use base64ct::Encoding;
use sha2::{Digest, Sha256};

#[derive(Clone, Debug)]
pub struct PasswordHasher {
    salt: String,
}

impl PasswordHasher {
    /// Creates a new password hasher using a salt
    ///
    /// # Arguments
    /// * `salt` - Salt for password hash
    pub fn new(salt: String) -> Self {
        PasswordHasher { salt }
    }
    /// Hash a password and a salt using SHA256, the hash is returned as base64 encoded string
    ///
    /// # Arguments
    /// * `password` - Password to hash
    pub fn hash_password(&self, password: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(password);
        hasher.update(&self.salt);
        let hash = hasher.finalize();

        base64ct::Base64::encode_string(&hash)
    }
}
