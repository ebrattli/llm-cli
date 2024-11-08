use super::llm::LLMClient;
use crate::config::Config;
use async_trait::async_trait;
use futures::{Stream, StreamExt};
use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE};
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::pin::Pin;

const OPENAI_API_URL: &str = "https://api.openai.com/v1/chat/completions";

#[derive(Debug, Serialize)]
struct OpenAIRequest {
    model: String,
    messages: Vec<Message>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stream: Option<bool>,
}

#[derive(Debug, Serialize)]
struct Message {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct OpenAIResponse {
    choices: Vec<Choice>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: MessageResponse,
}

#[derive(Debug, Deserialize)]
struct MessageResponse {
    content: String,
}

#[derive(Debug, Deserialize)]
struct OpenAIStreamResponse {
    choices: Vec<StreamChoice>,
}

#[derive(Debug, Deserialize)]
struct StreamChoice {
    delta: DeltaContent,
}

#[derive(Debug, Deserialize)]
struct DeltaContent {
    #[serde(default)]
    content: String,
}

#[derive(Clone)]
pub struct OpenAIClient {
    api_key: String,
    client: reqwest::Client,
}

impl OpenAIClient {
    pub fn new(api_key: String) -> Self {
        OpenAIClient {
            api_key,
            client: reqwest::Client::new(),
        }
    }

    fn build_headers(&self) -> Result<HeaderMap, Box<dyn Error + Send + Sync>> {
        let mut headers = HeaderMap::new();
        headers.insert(
            "Authorization",
            HeaderValue::from_str(&format!("Bearer {}", self.api_key))?,
        );
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        Ok(headers)
    }

    fn build_request(&self, message: &str, config: &Config, stream: bool) -> OpenAIRequest {
        let mut messages = Vec::new();
        
        if let Some(prompt) = &config.system_prompt {
            messages.push(Message {
                role: "system".to_string(),
                content: prompt.to_string(),
            });
        }

        messages.push(Message {
            role: "user".to_string(),
            content: message.to_string(),
        });

        OpenAIRequest {
            model: config.openai.default_model.clone(),
            messages,
            stream: Some(stream),
        }
    }
}

#[async_trait]
impl LLMClient for OpenAIClient {
    async fn send_message(
        &self,
        message: &str,
        config: &Config,
    ) -> Result<String, Box<dyn Error + Send + Sync>> {
        let request = self.build_request(message, config, false);
        let headers = self.build_headers()?;

        let response = self
            .client
            .post(OPENAI_API_URL)
            .headers(headers)
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(format!("API request failed: {}", error_text).into());
        }

        let response_body: OpenAIResponse = response.json().await?;
        Ok(response_body
            .choices
            .first()
            .map(|choice| choice.message.content.clone())
            .unwrap_or_default())
    }

    async fn send_message_streaming(
        &self,
        message: &str,
        config: &Config,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<String, Box<dyn Error + Send + Sync>>> + Send>>, Box<dyn Error + Send + Sync>> {
        let request = self.build_request(message, config, true);
        let headers = self.build_headers()?;

        let response = self
            .client
            .post(OPENAI_API_URL)
            .headers(headers)
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(format!("API request failed: {}", error_text).into());
        }

        let stream = response
            .bytes_stream()
            .map(move |result| {
                let bytes = result.map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;
                let text = String::from_utf8(bytes.to_vec())
                    .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;
                
                let mut response_text = String::new();
                for line in text.lines() {
                    if line.starts_with("data: ") {
                        let json_str = &line["data: ".len()..];
                        if json_str == "[DONE]" {
                            continue;
                        }
                        if let Ok(response) = serde_json::from_str::<OpenAIStreamResponse>(json_str) {
                            if let Some(choice) = response.choices.first() {
                                let content = choice.delta.content.replace("\\n", "\n");
                                if !content.is_empty() {
                                    if !response_text.is_empty() && !response_text.ends_with('\n') {
                                        response_text.push(' ');
                                    }
                                    response_text.push_str(&content);
                                }
                            }
                        }
                    }
                }
                Ok(response_text)
            })
            .filter(|result| futures::future::ready(result.as_ref().map(|s| !s.is_empty()).unwrap_or(true)));

        Ok(Box::pin(stream))
    }
}
