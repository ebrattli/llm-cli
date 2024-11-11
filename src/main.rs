use clap::Parser;
use dotenv::dotenv;
use llm_cli::cli::{run, Args};
use std::process;

#[tokio::main]
async fn main() {
    dotenv().ok();
    let args = Args::parse();
    if args.debug {
        env_logger::Builder::from_default_env()
            .filter_level(log::LevelFilter::Debug)
            .init();
    } else {
        env_logger::Builder::from_default_env()
            .filter_level(log::LevelFilter::Error)
            .init();
    }

    if let Err(e) = run(args).await {
        println!();
        eprintln!("Error: {}", e);
        process::exit(1);
    }
}
