use regex::Regex;
use syntect::easy::HighlightLines;
use syntect::highlighting::{ThemeSet, Style};
use syntect::parsing::SyntaxSet;
use syntect::util::{as_24_bit_terminal_escaped, LinesWithEndings};
use termcolor::{BufferWriter, Color, ColorChoice, ColorSpec, StandardStream, WriteColor};
use std::io::Write;

pub struct OutputFormatter {
    syntax_set: SyntaxSet,
    theme_set: ThemeSet,
    writer: BufferWriter,
}

impl OutputFormatter {
    pub fn new() -> Self {
        Self {
            syntax_set: SyntaxSet::load_defaults_newlines(),
            theme_set: ThemeSet::load_defaults(),
            writer: BufferWriter::stdout(ColorChoice::Always),
        }
    }

    pub fn create_writer(&self) -> StandardStream {
        StandardStream::stdout(ColorChoice::Always)
    }

    pub fn write_output<W: WriteColor + Write>(&self, text: &str, writer: &mut W) {
        let code_block_regex = Regex::new(r"```([^\n]*)\n([\s\S]*?)```").unwrap();
        let mut last_match_end = 0;

        for cap in code_block_regex.captures_iter(text) {
            // Write text before code block with regular formatting
            let match_start = cap.get(0).unwrap().start();
            if match_start > last_match_end {
                self.write_regular_text(
                    &text[last_match_end..match_start],
                    writer
                );
            }

            // Write code block with syntax highlighting
            let language = cap.get(1).unwrap().as_str();
            let code = cap.get(2).unwrap().as_str().trim();
            
            // Fix common code formatting issues
            let fixed_code = self.fix_code_formatting(code, language);
            self.write_code_block(&fixed_code, language, writer);

            last_match_end = cap.get(0).unwrap().end();
        }

        // Write remaining text after last code block
        if last_match_end < text.len() {
            self.write_regular_text(
                &text[last_match_end..],
                writer
            );
        }
    }

    fn fix_code_formatting(&self, code: &str, language: &str) -> String {
        let mut fixed = code.to_string();

        // Fix escaped newlines
        fixed = fixed.replace("\\n", "\n");

        // Fix header includes in C code
        if language == "c" {
            fixed = fixed.replace(".h>", "h>");
            fixed = fixed.replace(".H>", "h>");
        }

        // Fix inconsistent newlines
        fixed = fixed.replace("\r\n", "\n");
        fixed = fixed.replace("\r", "\n");

        // Remove trailing whitespace from lines
        fixed = fixed.lines()
            .map(|line| line.trim_end())
            .collect::<Vec<_>>()
            .join("\n");

        // Ensure single newline at end
        if !fixed.ends_with('\n') {
            fixed.push('\n');
        }

        fixed
    }

    fn write_code_block<W: WriteColor + Write>(&self, code: &str, language: &str, writer: &mut W) {
        let theme = &self.theme_set.themes["base16-ocean.dark"];
        
        // Try to determine the syntax in this order:
        // 1. From the specified language
        // 2. By first line detection
        // 3. Fallback to a best guess based on common patterns
        let syntax = if !language.is_empty() {
            self.syntax_set.find_syntax_by_token(language)
        } else {
            None
        }.or_else(|| {
            // Try to detect from first line
            self.syntax_set.find_syntax_by_first_line(code)
        }).or_else(|| {
            // Fallback detection based on common patterns
            if code.contains("function") || code.contains("var") || code.contains("let") || code.contains("const") {
                self.syntax_set.find_syntax_by_token("js")
            } else if code.contains("#include") || code.contains("int main") {
                self.syntax_set.find_syntax_by_token("c")
            } else if code.contains("def ") || code.contains("import ") {
                self.syntax_set.find_syntax_by_token("python")
            } else if code.contains("<?php") || code.contains("<?=") {
                self.syntax_set.find_syntax_by_token("php")
            } else if code.contains("<html") || code.contains("<!DOCTYPE") {
                self.syntax_set.find_syntax_by_token("html")
            } else if code.contains("sudo ") || code.contains("apt ") || code.contains("./") {
                self.syntax_set.find_syntax_by_token("bash")
            } else {
                None
            }
        }).unwrap_or_else(|| self.syntax_set.find_syntax_plain_text());

        let mut h = HighlightLines::new(syntax, theme);
        
        // Calculate the maximum line length for proper padding
        let max_line_length = code.lines()
            .map(|line| line.chars().count())
            .max()
            .unwrap_or(0);
        let border_width = std::cmp::max(max_line_length + 4, 80); // minimum width of 80

        // Add a fancy border at the top with language
        let lang_display = if !language.is_empty() {
            format!("[ {} ]", language.to_uppercase())
        } else if let Some(name) = syntax.name.split_whitespace().next() {
            format!("[ {} ]", name.to_uppercase())
        } else {
            String::new()
        };

        // Reset colors before starting
        writer.reset().unwrap();

        // Top border
        let mut blue_bold = ColorSpec::new();
        blue_bold.set_fg(Some(Color::Cyan)).set_bold(true);
        writer.set_color(&blue_bold).unwrap();
        writeln!(writer, "╭{}╮", "─".repeat(border_width - 2)).unwrap();

        if !lang_display.is_empty() {
            write!(writer, "│ ").unwrap();
            let mut yellow_bold = ColorSpec::new();
            yellow_bold.set_fg(Some(Color::Yellow)).set_bold(true);
            writer.set_color(&yellow_bold).unwrap();
            write!(writer, "{}", lang_display).unwrap();
            writer.set_color(&blue_bold).unwrap();
            writeln!(writer, "{} │", " ".repeat(border_width - lang_display.len() - 3)).unwrap();
            writeln!(writer, "├{}┤", "─".repeat(border_width - 2)).unwrap();
        }

        // Add the highlighted code with proper padding
        for line in LinesWithEndings::from(code) {
            writer.set_color(&blue_bold).unwrap();
            write!(writer, "│ ").unwrap();
            
            let ranges: Vec<(Style, &str)> = h.highlight_line(line.trim_end(), &self.syntax_set).unwrap();
            let escaped = as_24_bit_terminal_escaped(&ranges[..], true);
            writer.reset().unwrap();
            write!(writer, "{}", escaped).unwrap();
            
            let padding = " ".repeat(border_width - line.chars().count() - 3);
            writer.set_color(&blue_bold).unwrap();
            write!(writer, "{} │\n", padding).unwrap();
        }

        // Bottom border
        writer.set_color(&blue_bold).unwrap();
        writeln!(writer, "╰{}╯", "─".repeat(border_width - 2)).unwrap();
        writer.reset().unwrap();
        writeln!(writer).unwrap();
    }

    fn write_regular_text<W: WriteColor + Write>(&self, text: &str, writer: &mut W) {
        let mut green_bold = ColorSpec::new();
        green_bold.set_fg(Some(Color::Green)).set_bold(true);
        
        let mut blue_bold = ColorSpec::new();
        blue_bold.set_fg(Some(Color::Cyan)).set_bold(true);
        
        let mut yellow_bold = ColorSpec::new();
        yellow_bold.set_fg(Some(Color::Yellow)).set_bold(true);

        for line in text.lines() {
            writer.reset().unwrap();
            
            if line.starts_with("Human:") {
                writer.set_color(&green_bold).unwrap();
            } else if line.starts_with("Assistant:") {
                writer.set_color(&blue_bold).unwrap();
            } else if line.trim().starts_with("**") && line.trim().ends_with("**") {
                writer.set_color(&yellow_bold).unwrap();
            }
            
            writeln!(writer, "{}", line).unwrap();
            writer.reset().unwrap();
        }
    }

    // Keep format_output for backward compatibility
    pub fn format_output(&self, text: &str) -> String {
        let mut buffer = self.writer.buffer();
        self.write_output(text, &mut buffer);
        String::from_utf8_lossy(&buffer.into_inner()).into_owned()
    }
}

impl Default for OutputFormatter {
    fn default() -> Self {
        Self::new()
    }
}
