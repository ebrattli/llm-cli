use serde::Deserialize;
use std::fs;
use std::path::Path;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub default_provider: String,
    pub system_prompt: Option<String>,
    pub claude: ProviderConfig,
    pub openai: ProviderConfig,
}

#[derive(Debug, Deserialize)]
pub struct ProviderConfig {
    pub default_model: String,
    pub max_tokens: u32,
}

impl Config {
    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        let config_path = Path::new("config.toml");
        let contents = fs::read_to_string(config_path)?;
        let config: Config = toml::from_str(&contents)?;
        Ok(config)
    }

    pub fn get_model_for_provider(&self, provider: &str) -> Option<String> {
        match provider {
            "claude" => Some(self.claude.default_model.clone()),
            "openai" => Some(self.openai.default_model.clone()),
            _ => None,
        }
    }

    pub fn get_max_tokens_for_provider(&self, provider: &str) -> Option<u32> {
        match provider {
            "claude" => Some(self.claude.max_tokens),
            "openai" => Some(self.openai.max_tokens),
            _ => None,
        }
    }
}
