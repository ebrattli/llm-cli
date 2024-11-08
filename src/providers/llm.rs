use async_trait::async_trait;
use futures::Stream;
use std::error::Error;
use std::pin::Pin;
use crate::config::Config;

#[async_trait]
pub trait LLMClient {
    // Legacy non-streaming method
    async fn send_message(
        &self,
        message: &str,
        config: &Config,
    ) -> Result<String, Box<dyn Error + Send + Sync>>;

    // New streaming method
    async fn send_message_streaming(
        &self,
        message: &str,
        config: &Config,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<String, Box<dyn Error + Send + Sync>>> + Send>>, Box<dyn Error + Send + Sync>>;
}
