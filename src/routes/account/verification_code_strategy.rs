use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier, password_hash::Salt};
use base64::prelude::*;
use hmac::{Hmac, Mac};
use rand::prelude::*;
use rand_chacha::ChaCha20Rng;
use sha3::Sha3_256;

#[derive(Debug)]
pub struct VerificationCodeStategy;

impl VerificationCodeStategy {
    /// Generate a verification code linked to an email with its encryption
    ///
    /// The code is a random 8 digits number.
    /// An encryption of the code is performed for later verification:
    ///     1. a random 16 bytes (128 bits) salt is generated,
    ///     2. a key is derived using the Argon2id scheme with the salt and the code as password,
    ///     3. a mac is computed using HMAC(key hash, email, SHA3-256)
    ///
    /// # Arguments
    /// * `email` - email to link the verification code to
    pub fn generate_verification_code(email: &str) -> Result<(u32, String), anyhow::Error> {
        let mut salt = [0u8; 16];
        let mut rng = ChaCha20Rng::from_os_rng();
        rng.fill_bytes(&mut salt);
        let base64_salt = BASE64_STANDARD_NO_PAD.encode(salt);
        let argon_salt = Salt::from_b64(&base64_salt).map_err(|e| anyhow::anyhow!("{e}"))?;

        let mut code: u32 = rng.random();
        // Code is up to 8 numbers
        code %= 100_000_000;
        let key = Argon2::default()
            .hash_password(&code.to_le_bytes(), argon_salt)
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        let key_hash = key
            .hash
            .ok_or(anyhow::anyhow!("Unable to extract hash from key"))?;

        let mut hmac: Hmac<Sha3_256> = Hmac::new_from_slice(key_hash.as_bytes())?;
        hmac.update(email.as_bytes());
        let mac = hmac.finalize().into_bytes();

        // Mac is 32 bytes
        // Key is a string of 97 bytes
        let mut cyphertext = [0u8; 129];
        cyphertext[..97].copy_from_slice(key.serialize().as_bytes());
        cyphertext[97..].copy_from_slice(&mac);

        Ok((code, BASE64_STANDARD_NO_PAD.encode(cyphertext)))
    }

    /// Verify a verification code, returns true if code is correct, false otherwise
    ///
    /// The code is verified against the Argon2id generated key.
    /// The mail is verified against the HMAC of the generated key hash, the email and using SHA3-256
    ///
    /// # Arguments
    /// * `code` - 8 digits secret code,
    /// * `email` - email to which the code is linked,
    /// * `cyphertext` - the compactified elements of the encryption of the code, previously generated
    pub fn verify_verification_code(
        code: u32,
        email: &str,
        cyphertext: &str,
    ) -> Result<bool, anyhow::Error> {
        let cyphertext_bytes = BASE64_STANDARD_NO_PAD.decode(cyphertext)?;
        if cyphertext_bytes.len() != 129 {
            return Err(anyhow::anyhow!(
                "Expected 129 bytes length string, got {}",
                cyphertext_bytes.len()
            ));
        }
        let (key, mac) = cyphertext_bytes.split_at(97);

        let password_hash =
            PasswordHash::new(std::str::from_utf8(key)?).map_err(|e| anyhow::anyhow!("{e}"))?;

        Argon2::default()
            .verify_password(&code.to_le_bytes(), &password_hash)
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        let mut hmac: Hmac<Sha3_256> = Hmac::new_from_slice(
            password_hash
                .hash
                .ok_or(anyhow::anyhow!("Unable to extract hash from key"))?
                .as_bytes(),
        )?;
        hmac.update(email.as_bytes());

        Ok(hmac.verify_slice(mac).is_ok())
    }
}

#[cfg(test)]
mod tests {
    use fake::{Fake, faker};

    use super::*;

    #[test]
    fn test_verification_code_encryption() {
        let email: String = faker::internet::en::SafeEmail().fake();
        let (code, cyphertext) =
            VerificationCodeStategy::generate_verification_code(&email).unwrap();
        assert!(
            VerificationCodeStategy::verify_verification_code(code, &email, &cyphertext).is_ok()
        );
    }
}
