use log::debug;

use super::args::Args;
use crate::{
    core::{conversation::ConversationManager, Config, Formatter, LLMError, Provider},
    providers::{claude::ClaudeClient, llm::LLMClient, openai::OpenAIClient, Message},
    tools::{CommandHistoryTool, ExecuteCommandTool, ToolRegistry},
};
use std::io::{self, Write};

/// Creates a new LLM client based on the specified provider
///
/// # Arguments
/// * `provider` - The name of the LLM provider to use
/// * `config` - The configuration for the client
/// * `debug` - Whether to output debug information
///
/// # Returns
/// A boxed LLM client implementing the `LLMClient` trait
fn create_llm_client(config: Config, debug: bool) -> Result<Box<dyn LLMClient>, LLMError> {
    match config.provider {
        Provider::Claude => {
            if debug {
                eprintln!("[DEBUG] Initializing Claude client");
            }
            let api_key = dotenv::var("ANTHROPIC_API_KEY")
                .or_else(|_| std::env::var("ANTHROPIC_API_KEY"))
                .map_err(|_| {
                    LLMError::ApiError(
                        "ANTHROPIC_API_KEY not set in .env or environment".to_string(),
                    )
                })?;
            Ok(Box::new(ClaudeClient::new(api_key, config)))
        }
        Provider::OpenAI => {
            if debug {
                eprintln!("[DEBUG] Initializing OpenAI client");
            }
            let api_key = dotenv::var("OPENAI_API_KEY")
                .or_else(|_| std::env::var("OPENAI_API_KEY"))
                .map_err(|_| {
                    LLMError::ApiError("OPENAI_API_KEY not set in .env or environment".to_string())
                })?;
            Ok(Box::new(OpenAIClient::new(api_key, config)))
        }
    }
}

pub async fn run(args: Args) -> Result<(), LLMError> {
    let _ = dotenv::dotenv();

    let query = args.query;
    if query.is_empty() {
        return Err(LLMError::ApiError("Query must not be empty".to_string()));
    }
    let mut config = Config::load()?;
    let enable_tools = args.enable_tools.unwrap_or(config.enable_tools);
    let max_steps = args.max_steps.unwrap_or(config.max_steps);

    if let Some(provider) = args.provider {
        config.update_provider(provider);
    }

    debug!(
        "[SETTINGS] provider: {:?}, tool_enabled: {enable_tools}, max_steps: {max_steps}",
        config.provider
    );

    let formatter = Formatter::new(std::mem::take(&mut config.theme));
    let client = create_llm_client(config, args.debug)?;
    let registry = enable_tools.then(|| {
        let mut registry = ToolRegistry::new();
        registry.register(ExecuteCommandTool);
        registry.register(CommandHistoryTool);
        registry
    });
    let mut conversation_manager = ConversationManager::new(client, registry, formatter);
    let mut stdout = io::stdout();
    let _ = conversation_manager
        .run(vec![Message::user(query)], max_steps, &mut stdout)
        .await?;

    // Ensure final newline
    writeln!(&mut stdout)?;
    Ok(())
}
