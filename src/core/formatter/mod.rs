mod codeblock_detector;
mod syntax_highlighter;

use codeblock_detector::StateTransition;
pub use codeblock_detector::{CodeBlockDetector, CodeBlockState};
use std::io::Write;
pub use syntax_highlighter::{SyntaxHighlighter, SyntaxHighlighting};

use crate::core::LLMError;

pub struct Formatter<H: SyntaxHighlighting> {
    code_block_detector: CodeBlockDetector,
    syntax_highlighter: H,
    code_block: CodeBlock,
    text_buffer: String,
}

impl Formatter<SyntaxHighlighter> {
    pub fn new(theme: Option<String>) -> Self {
        Self::new_with_highlighter(SyntaxHighlighter::new(theme))
    }
}

impl Default for Formatter<SyntaxHighlighter> {
    fn default() -> Self {
        Self::new(None)
    }
}

struct CodeBlock {
    language: Option<String>,
    buffer: String,
    is_first_line: bool,
    formatting_active: bool,
}

impl CodeBlock {
    fn new(language: Option<String>) -> Self {
        Self {
            language,
            buffer: String::with_capacity(200),
            is_first_line: true,
            formatting_active: false,
        }
    }

    fn clear(&mut self) {
        self.language = None;
        self.buffer.clear();
        self.is_first_line = true;
        self.formatting_active = false;
    }
}

impl<H: SyntaxHighlighting> Formatter<H> {
    pub fn new_with_highlighter(syntax_highlighter: H) -> Self {
        Self {
            code_block_detector: CodeBlockDetector::new(),
            syntax_highlighter,
            code_block: CodeBlock::new(None),
            text_buffer: String::with_capacity(64),
        }
    }

    pub fn format_chunk<W: Write>(&mut self, writer: &mut W, chunk: &str) -> Result<(), LLMError> {
        chunk.chars().try_for_each(|c| -> Result<(), LLMError> {
            if c == '`' {
                self.code_block_detector.handle_backtick();
            } else {
                let new_state = self.code_block_detector.evaluate_code_block_state();
                if let StateTransition::Transition(new_state) = new_state {
                    self.flush_previous_state_buffer(writer, new_state)?;
                }
                if let StateTransition::NoTransition(unused_backticks) = new_state {
                    self.append_backticks_to_buffer(unused_backticks);
                }
                match self.code_block_detector.state {
                    CodeBlockState::Normal => self.text_buffer.push(c),
                    CodeBlockState::CodeBlock | CodeBlockState::InlineCode => {
                        self.write_code_block(writer, c)?;
                    }
                }
            }
            Ok(())
        })?;

        self.flush_buffer(writer)
    }

    fn append_backticks_to_buffer(&mut self, count: usize) {
        let target = match self.code_block_detector.state {
            CodeBlockState::Normal => &mut self.text_buffer,
            CodeBlockState::CodeBlock | CodeBlockState::InlineCode => &mut self.code_block.buffer,
        };

        target.push_str("`".repeat(count).as_str());
    }

    fn write_text<W: Write>(writer: &mut W, content: &str) -> Result<(), LLMError> {
        writer
            .write_all(content.as_bytes())
            .map_err(|e| LLMError::IOError(e.to_string()))
    }

    fn highlight_and_write<W: Write>(&mut self, writer: &mut W) -> Result<(), LLMError> {
        self.code_block.formatting_active = true;
        let highlighted_code = self
            .syntax_highlighter
            .highlight_code(&self.code_block.buffer, self.code_block.language.as_deref())?;
        Self::write_text(writer, &highlighted_code)
    }

    fn write_code_block<W: Write>(&mut self, writer: &mut W, c: char) -> Result<(), LLMError> {
        self.code_block.buffer.push(c);

        if c == '\n' {
            if self.code_block.is_first_line {
                self.code_block.is_first_line = false;
                let language = self.code_block.buffer.trim();
                if self.syntax_highlighter.is_valid_language(language) {
                    self.code_block.language = Some(language.to_string());
                } else {
                    self.highlight_and_write(writer)?;
                }
            } else {
                self.highlight_and_write(writer)?;
            }

            self.code_block.buffer.clear();
        }
        Ok(())
    }

    fn flush_code_block_buffer<W: Write>(&mut self, writer: &mut W) -> Result<(), LLMError> {
        if !self.code_block.buffer.is_empty() {
            self.highlight_and_write(writer)?;
        }
        self.unset_highlighting(writer)?;
        self.code_block.clear();
        Ok(())
    }

    fn flush_buffer<W: Write>(&mut self, writer: &mut W) -> Result<(), LLMError> {
        if !self.text_buffer.is_empty() {
            Self::write_text(writer, &self.text_buffer)?;
            self.text_buffer.clear();
        }
        Ok(())
    }

    fn flush_previous_state_buffer<W: Write>(
        &mut self,
        writer: &mut W,
        new_state: CodeBlockState,
    ) -> Result<(), LLMError> {
        match new_state {
            CodeBlockState::Normal => self.flush_code_block_buffer(writer),
            CodeBlockState::CodeBlock | CodeBlockState::InlineCode => self.flush_buffer(writer),
        }
    }

    pub fn finish<W: Write>(&mut self, writer: &mut W) -> Result<(), LLMError> {
        if !self.code_block.buffer.is_empty() {
            self.highlight_and_write(writer)?;
        }
        if self.code_block.formatting_active {
            self.unset_highlighting(writer)?;
        }
        self.flush_buffer(writer)
    }

    fn unset_highlighting<W: Write>(&mut self, writer: &mut W) -> Result<(), LLMError> {
        self.code_block.formatting_active = false;
        writer
            .write_all(self.syntax_highlighter.unset_code())
            .map_err(|e| LLMError::IOError(e.to_string()))
    }
}

#[cfg(test)]
struct TestSyntaxHighlighter;

#[cfg(test)]
impl SyntaxHighlighting for TestSyntaxHighlighter {
    fn highlight_code(&self, content: &str, _: Option<&str>) -> Result<String, LLMError> {
        Ok(content.to_uppercase())
    }

    fn is_valid_language(&self, language: &str) -> bool {
        matches!(language, "rust" | "python" | "javascript")
    }

    fn unset_code(&self) -> &[u8] {
        b"|"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn format_text(text: &str) -> String {
        let mut formatter = Formatter::new_with_highlighter(TestSyntaxHighlighter);
        let mut output = Vec::new();
        formatter.format_chunk(&mut output, text).unwrap();
        formatter.finish(&mut output).unwrap();
        String::from_utf8(output).unwrap()
    }

    fn format_chunks(chunks: &[&str]) -> String {
        let mut formatter = Formatter::new_with_highlighter(TestSyntaxHighlighter);
        let mut output = Vec::new();
        for chunk in chunks {
            formatter.format_chunk(&mut output, chunk).unwrap();
        }
        formatter.finish(&mut output).unwrap();
        String::from_utf8(output).unwrap()
    }

    #[test]
    fn test_normal_text() {
        assert_eq!(format_text("Hello world"), "Hello world");
    }

    #[test]
    fn test_inline_code() {
        assert_eq!(format_text("Hello `code` world"), "Hello CODE| world");
    }

    #[test]
    fn test_code_block() {
        let input = "```rust\nfn main() {\n    println!(\"Hello\");\n}\n```";
        let expected = "FN MAIN() {\n    PRINTLN!(\"HELLO\");\n}\n|";
        assert_eq!(format_text(input), expected);
    }

    #[test]
    fn test_code_block_across_chunks() {
        let chunks = &[
            "Here's some code:\n```ru",
            "st\nfn main() {\n",
            "    println!(\"Hello\");\n}\n```",
        ];
        let expected = "Here's some code:\nFN MAIN() {\n    PRINTLN!(\"HELLO\");\n}\n|";
        assert_eq!(format_chunks(chunks), expected);
    }

    #[test]
    fn test_very_small_chunks() {
        let chunks = &["aaa", "`", "``", "bbb", "``", "`", "aaa"];
        assert_eq!(format_chunks(chunks), "aaaBBB|aaa");
    }

    #[test]
    fn test_unrecognized_language() {
        let input = "```invalid\nsome code\n```";
        let expected = "INVALID\nSOME CODE\n|";
        assert_eq!(format_text(input), expected);
    }

    #[test]
    fn test_empty_content() {
        assert_eq!(format_text(""), "");
    }

    // Because we receive the content in chunks streaming, we can't guarantee that the code block
    // will be closed in the same chunk it was opened. Currently we prioritize streaming the output
    // as soon as possible, so we don't wait for the code block to be closed before highlighting it.
    // This means that the code block will be highlighted even if it's not closed.
    #[test]
    #[ignore]
    fn test_single_backtick() {
        assert_eq!(format_text("a`b"), "a`b");
    }

    #[test]
    fn test_multiple_code_blocks() {
        let input = "```rust\nfn main() {}\n```\ntext\n```python\ndef main():\n    pass\n```";
        let expected = "FN MAIN() {}\n|\ntext\nDEF MAIN():\n    PASS\n|";
        assert_eq!(format_text(input), expected);
    }

    #[test]
    fn test_nested_code_blocks() {
        let input = "```rust\nfn main() {\n    println!(\"`Hello`\");\n}\n```";
        let expected = "FN MAIN() {\n    PRINTLN!(\"`HELLO`\");\n}\n|";
        assert_eq!(format_text(input), expected);
    }
}
