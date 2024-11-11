use std::fmt::Display;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::core::error::ToolError;

/// Represents a tool call with its identifier, name, and arguments
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    /// Unique identifier for the tool call
    pub id: String,
    /// Name of the tool being called
    pub name: String,
    /// Arguments passed to the tool as a JSON value
    pub arguments: Value,
}

impl Display for ToolCall {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}({})", self.name, self.arguments)
    }
}

/// Defines a tool's interface including its name, description, and parameter schema
#[derive(Debug, Clone, Serialize)]
pub struct ToolDefinition {
    /// Name of the tool
    pub name: String,
    /// Description of what the tool does
    pub description: String,
    /// JSON schema defining the tool's parameters
    pub parameters: Value,
}

/// Trait that must be implemented by all tools
#[async_trait]
pub trait Tool: Send + Sync {
    /// Returns the tool's definition including its name, description, and parameter schema
    fn definition(&self) -> ToolDefinition;

    /// Executes the tool with the provided arguments
    ///
    /// # Arguments
    /// * `arguments` - JSON value containing the tool's arguments
    ///
    /// # Returns
    /// * `Result<Value, String>` - JSON value result or error message
    async fn execute(&self, arguments: &Value) -> Result<Value, ToolError>;
}
