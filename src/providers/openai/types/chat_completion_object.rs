#![allow(dead_code)]

use super::message::{FinishReason, Message};
use super::shared::{LogProbs, Usage};
use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, Deserialize)]
pub struct ChatCompletionObject<'a> {
    pub id: String,
    pub object: String,
    pub created: u64,
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_fingerprint: Option<String>,
    #[serde(borrow)]
    pub choices: Vec<Choice<'a>>,
    pub usage: Usage,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Choice<'a> {
    pub finish_reason: Option<FinishReason>,
    pub index: u32,
    #[serde(borrow)]
    pub message: Message<'a>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logprobs: Option<LogProbs>,
}
