use std::fmt::Debug;

use anyhow::anyhow;
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier, password_hash::Salt};
use base64::{Engine, prelude::BASE64_STANDARD_NO_PAD};
use fake::{Dummy, Fake, faker};
use rand::{RngCore, SeedableRng};
use rand_chacha::ChaCha20Rng;
use serde::{Deserialize, de::Visitor};

// ##################################################
// #################### PASSWORD ####################
// ##################################################

/// This type is meant to be used internally and in incoming IO requests (body payloads)
#[derive(Clone, PartialEq, Eq)]
pub struct Password(String);

#[derive(Debug)]
pub enum PasswordError {
    Empty,
    InvalidPassword(String),
}

impl Password {
    /// Creates a new `Password` instance after validating the provided string.
    ///
    /// # Arguments
    ///
    /// * `v` - A string slice representing the password to validate and wrap.
    ///
    /// # Validation Rules
    ///
    /// - Password must not be empty.
    /// - Password length must be at least 10 characters and at most 40 characters.
    /// - Password must contain at least two uppercase letters.
    /// - Password must contain at least two numbers.
    /// - Password must contain at least two special characters (characters that are not letters or numbers).
    ///
    /// # Errors
    ///
    /// Returns a `PasswordError` if any of the validation rules are not met:
    /// - `PasswordError::Empty` if the password is empty.
    /// - `PasswordError::InvalidPassword` with a descriptive message if any other rule is violated.
    pub fn new(v: &str) -> Result<Self, PasswordError> {
        if v.is_empty() {
            return Err(PasswordError::Empty);
        }
        // Password must be at least 10 characters long, at most 40 characters long
        if v.len() < 10 || v.len() > 40 {
            return Err(PasswordError::InvalidPassword(
                "password length must be at least 10 characters and at most 40 characters"
                    .to_string(),
            ));
        }
        // Password must contain:
        //  - at least two capital letters,
        //  - at least two numbers,
        //  - at least two special characters (not number nor letter)
        let mut uppercase_count = 0;
        let mut number_count = 0;
        let mut special_count = 0;

        for c in v.chars() {
            if c.is_ascii_uppercase() {
                uppercase_count += 1;
            } else if c.is_ascii_digit() {
                number_count += 1;
            } else if !c.is_ascii_alphanumeric() {
                special_count += 1;
            }
        }

        if uppercase_count < 2 {
            return Err(PasswordError::InvalidPassword(
                "password must contain at least two uppercase letters".to_string(),
            ));
        }
        if number_count < 2 {
            return Err(PasswordError::InvalidPassword(
                "password must contain at least two numbers".to_string(),
            ));
        }
        if special_count < 2 {
            return Err(PasswordError::InvalidPassword(
                "password must contain at least two special characters".to_string(),
            ));
        }

        Ok(Password(v.to_string()))
    }

    /// Hash a password using the Argon2id algorithm. The returned string is a argon2-formatted hash.
    ///
    /// # Arguments
    /// * `password` - Password to hash
    pub fn hash(&self) -> Result<String, anyhow::Error> {
        let mut salt = [0u8; 16];
        let mut rng = ChaCha20Rng::from_os_rng();
        rng.fill_bytes(&mut salt);
        let base64_salt = BASE64_STANDARD_NO_PAD.encode(salt);
        let argon_salt = Salt::from_b64(&base64_salt).map_err(|e| {
            anyhow!(e).context("failed to build Salt struct from base64 salt string")
        })?;
        Argon2::default()
            .hash_password(self.0.as_bytes(), argon_salt)
            .map_err(|e| anyhow!(e).context("failed to hash password"))
            .map(|v| v.to_string())
    }

    /// Verify a password validity against an Argon2id formatted key
    ///
    /// # Arguments
    /// * `password` - Password to hash
    /// * `password_hash` - Argon2id formatted key
    pub fn verify(&self, password_hash: &str) -> Result<(), anyhow::Error> {
        let password_hash = PasswordHash::new(password_hash).map_err(|e| {
            anyhow!(e).context("failed to build PasswordHash struct from raw string")
        })?;
        Argon2::default()
            .verify_password(self.0.as_bytes(), &password_hash)
            .map_err(|e| anyhow!(e).context("failed to verify password"))
    }
}

impl std::fmt::Display for Password {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "******")
    }
}

impl Debug for Password {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "******")
    }
}

impl<T> Dummy<T> for Password {
    fn dummy_with_rng<R: rand::Rng + ?Sized>(_: &T, rng: &mut R) -> Self {
        let mut password: String = faker::internet::en::Password(10..36).fake_with_rng(rng);
        password += "{&";
        password += "24";
        Password(password)
    }
}

struct PasswordVisitor;

impl<'de> Visitor<'de> for PasswordVisitor {
    type Value = Password;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("a valid password of 10 to 40 characters. Must contain at least 2 special characters, 2 digits and 2 capital letters")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Password::new(v).map_err(|e| match e {
            PasswordError::Empty => serde::de::Error::custom("password must not be empty"),
            PasswordError::InvalidPassword(reason) => serde::de::Error::custom(reason),
        })
    }

    fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        self.visit_str(v.as_str())
    }
}

impl<'de> Deserialize<'de> for Password {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_string(PasswordVisitor)
    }
}
