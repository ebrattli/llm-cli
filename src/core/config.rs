use crate::core::LLMError;
use clap::ValueEnum;
use serde::Deserialize;
use std::fs;
use std::path::Path;

include!(concat!(env!("OUT_DIR"), "/config_embedded.rs"));

#[derive(Debug, Deserialize, Clone)]
pub struct ProviderConfig {
    pub default_model: String,
    pub max_tokens: u32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub provider: Provider,
    pub system_prompt: Option<String>,
    pub claude: ProviderConfig,
    pub openai: ProviderConfig,
    pub enable_tools: bool,
    pub max_steps: u32,
    pub theme: Option<String>,
}

#[derive(Clone, Debug, Deserialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    #[value(name = "claude")]
    Claude,
    #[value(name = "openai")]
    OpenAI,
}

impl Default for Config {
    fn default() -> Self {
        toml::from_str(DEFAULT_CONFIG).expect("Invalid default config")
    }
}

impl Config {
    pub fn load() -> Result<Self, LLMError> {
        let config_path = Path::new("config.toml");
        if config_path.exists() {
            let contents = fs::read_to_string(config_path)
                .map_err(|e| LLMError::ConfigError(format!("Failed to read config file: {e}")))?;

            toml::from_str(&contents)
                .map_err(|e| LLMError::ConfigError(format!("Failed to parse config file: {e}")))
        } else {
            Ok(Self::default())
        }
    }

    pub fn update_provider(&mut self, new_provider: Provider) {
        self.provider = new_provider;
    }

    pub fn get_model(&self) -> &str {
        match self.provider {
            Provider::Claude => &self.claude.default_model,
            Provider::OpenAI => &self.openai.default_model,
        }
    }

    pub const fn get_max_tokens(&self) -> u32 {
        match self.provider {
            Provider::Claude => self.claude.max_tokens,
            Provider::OpenAI => self.openai.max_tokens,
        }
    }
}
