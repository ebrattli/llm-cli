use clap::Parser;

use crate::core::Provider;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// Enable tool usage
    #[arg(long)]
    pub enable_tools: Option<bool>,

    /// Maximum number of tool execution steps
    #[arg(long)]
    pub max_steps: Option<u32>,

    /// Your query to the LLM
    #[arg()]
    pub query: String,

    /// LLM provider to use (openai or claude)
    #[arg(short, long, value_enum)]
    pub provider: Option<Provider>,

    /// Enable debug output
    #[arg(short, long, default_value = "false")]
    pub debug: bool,
}
