use std::env;
use std::fs;
use std::path::Path;

fn main() {
    println!("cargo:rerun-if-changed=config.toml");

    let config_path = Path::new("config.toml");
    let config_content = fs::read_to_string(config_path).expect("Failed to read config.toml");

    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("config_embedded.rs");

    fs::write(
        dest_path,
        format!(
            "pub const DEFAULT_CONFIG: &str = r#\"{}\"#;",
            config_content
        ),
    )
    .expect("Failed to write config string");
}
