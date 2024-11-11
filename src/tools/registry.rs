use serde_json::Value;
use std::collections::HashMap;

use crate::core::error::ToolError;

use super::types::{Tool, ToolDefinition};

/// Registry for managing and looking up available tools
#[derive(Default)]
pub struct ToolRegistry {
    tools: HashMap<String, Box<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    pub fn register(&mut self, tool: impl Tool + 'static) {
        let def = tool.definition();
        self.tools.insert(def.name, Box::new(tool));
    }

    pub fn get_tool_definitions(&self) -> Vec<ToolDefinition> {
        self.tools.values().map(|t| t.definition()).collect()
    }

    pub fn get_tool(&self, name: &str) -> Option<&dyn Tool> {
        Some(self.tools.get(name)?.as_ref())
    }

    pub async fn execute_tool(&self, name: &str, arguments: &Value) -> Result<Value, ToolError> {
        let tool = self
            .get_tool(name)
            .ok_or_else(|| ToolError::ToolNotFound(name.to_string()))?;
        tool.execute(arguments).await
    }
}
