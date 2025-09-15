use super::newtypes;
use async_trait::async_trait;
use tracing::warn;

#[async_trait]
pub trait MailingService: Send + Sync {
    async fn send_email(&self, email: &newtypes::Email, content: &str)
    -> Result<(), anyhow::Error>;
}

#[derive(Debug, Clone)]
pub struct ToBeImplementedMailingService;

#[async_trait]
impl MailingService for ToBeImplementedMailingService {
    async fn send_email(
        &self,
        _email: &newtypes::Email,
        content: &str,
    ) -> Result<(), anyhow::Error> {
        warn!("THIS LOG IS MEANT TO BE DELETED IN THE FUTURE -- Email content is {content}");
        Ok(())
    }
}
