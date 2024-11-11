use crate::core::LLMError;
use syntect::{
    easy::HighlightLines,
    highlighting::{Theme, ThemeSet},
    parsing::SyntaxSet,
    util::{as_24_bit_terminal_escaped, LinesWithEndings},
};

pub trait SyntaxHighlighting {
    fn highlight_code(&self, content: &str, language: Option<&str>) -> Result<String, LLMError>;
    fn is_valid_language(&self, language: &str) -> bool;
    fn unset_code(&self) -> &[u8] {
        b"\x1b[0m"
    }
}

pub struct SyntaxHighlighter {
    syntax_set: SyntaxSet,
    theme: Theme,
}

impl SyntaxHighlighter {
    pub fn new(theme_name: Option<String>) -> Self {
        // Load the default syntax definitions for newlines-based parsing
        let syntax_set = SyntaxSet::load_defaults_newlines();
        // Load the default themes
        let theme_set = ThemeSet::load_defaults();

        // Use specified theme or fallback to base16-ocean.dark
        let theme = theme_name
            .and_then(|name| theme_set.themes.get(&name).cloned())
            .unwrap_or_else(|| theme_set.themes["base16-ocean.dark"].clone());

        Self { syntax_set, theme }
    }
}

impl Default for SyntaxHighlighter {
    fn default() -> Self {
        Self::new(None)
    }
}

impl SyntaxHighlighting for SyntaxHighlighter {
    fn highlight_code(&self, content: &str, language: Option<&str>) -> Result<String, LLMError> {
        // Determine the syntax based on the provided language, fallback to plain text
        let syntax = language
            .and_then(|lang| self.syntax_set.find_syntax_by_token(lang))
            .unwrap_or_else(|| self.syntax_set.find_syntax_plain_text());

        let mut highlighter = HighlightLines::new(syntax, &self.theme);
        let mut result = String::new();
        // Process each line (content is one line, but LinesWithEndings handles it)
        for line in LinesWithEndings::from(content) {
            let regions = highlighter
                .highlight_line(line, &self.syntax_set)
                .map_err(|e| LLMError::FormatError(format!("Syntax highlighting error: {e}")))?;

            // Convert highlighted regions to ANSI-escaped string
            let escaped = as_24_bit_terminal_escaped(&regions, true);
            result.push_str(&escaped);
        }
        Ok(result)
    }

    fn is_valid_language(&self, language: &str) -> bool {
        self.syntax_set.find_syntax_by_token(language).is_some()
    }
}
