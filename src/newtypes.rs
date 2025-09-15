use fake::{Dummy, Fake, faker};
use sqlx::{Database, Decode, Encode};
use validator::ValidateEmail;

// #[derive(Clone, Debug, PartialEq, Eq)]
// pub struct Password(String);

// impl Password {
//     pub fn new(v: &str) -> Self {
//         Self(v.to_owned())
//     }
// }

// impl std::fmt::Display for Password {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         write!(f, "******")
//     }
// }

// ###############################################
// #################### EMAIL ####################
// ###############################################

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Email(String);

#[derive(Debug)]
pub enum EmailError {
    Empty,
    InvalidFormat,
}
impl Email {
    pub fn new(v: &str) -> Result<Self, EmailError> {
        let trimmed = v.trim();
        if trimmed.is_empty() {
            return Err(EmailError::Empty);
        }
        if !trimmed.validate_email() {
            return Err(EmailError::InvalidFormat);
        }
        Ok(Self(trimmed.to_lowercase()))
    }

    pub fn new_unchecked(v: &str) -> Self {
        Self(v.to_owned())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<DB> sqlx::Type<DB> for Email
where
    DB: Database,
    String: sqlx::Type<DB>,
{
    fn type_info() -> DB::TypeInfo {
        String::type_info()
    }
}

impl<'q, DB> Encode<'q, DB> for Email
where
    DB: Database,
    String: Encode<'q, DB>,
{
    // Required method
    fn encode_by_ref(
        &self,
        buf: &mut <DB as Database>::ArgumentBuffer<'q>,
    ) -> Result<sqlx::encode::IsNull, Box<dyn std::error::Error + Sync + Send>> {
        <String as Encode<'q, DB>>::encode_by_ref(&self.0, buf)
    }
}

impl<'r, DB: Database> Decode<'r, DB> for Email
where
    // we want to delegate some of the work to string decoding so let's make sure strings
    // are supported by the database
    &'r str: Decode<'r, DB>,
{
    fn decode(
        value: <DB as Database>::ValueRef<'r>,
    ) -> Result<Email, Box<dyn std::error::Error + 'static + Send + Sync>> {
        // the interface of ValueRef is largely unstable at the moment
        // so this is not directly implementable

        // however, you can delegate to a type that matches the format of the type you want
        // to decode (such as a UTF-8 string)

        let value = <&str as Decode<DB>>::decode(value)?;

        Ok(Email::new_unchecked(value))
    }
}

impl<T> Dummy<T> for Email {
    fn dummy_with_rng<R: rand::Rng + ?Sized>(_: &T, rng: &mut R) -> Self {
        let email: String = faker::internet::en::SafeEmail().fake_with_rng(rng);
        Email::new(&email).unwrap()
    }
}

impl std::fmt::Display for Email {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
