#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodeBlockState {
    Normal,
    CodeBlock,
    InlineCode,
}

#[derive(Debug, PartialEq, Eq)]
pub enum StateTransition {
    Transition(CodeBlockState),
    NoTransition(usize),
}

/// Detects and tracks code block state in markdown text
#[derive(Debug)]
pub struct CodeBlockDetector {
    pending_backticks: usize,
    pub state: CodeBlockState,
}

impl Default for CodeBlockDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl CodeBlockDetector {
    pub const fn new() -> Self {
        Self {
            pending_backticks: 0,
            state: CodeBlockState::Normal,
        }
    }

    #[inline]
    pub fn handle_backtick(&mut self) {
        self.pending_backticks += 1;
    }

    #[inline]
    pub fn evaluate_code_block_state(&mut self) -> StateTransition {
        let new_state = match (self.pending_backticks, self.state) {
            (1, CodeBlockState::Normal) => Some(CodeBlockState::InlineCode),
            (3, CodeBlockState::Normal) => Some(CodeBlockState::CodeBlock),
            (1, CodeBlockState::InlineCode) | (3, CodeBlockState::CodeBlock) => {
                Some(CodeBlockState::Normal)
            }
            _ => None,
        }
        .inspect(|&new_state| {
            self.state = new_state;
        });

        let state_transition = new_state.map_or(
            StateTransition::NoTransition(self.pending_backticks),
            StateTransition::Transition,
        );
        self.pending_backticks = 0;
        state_transition
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_backtick_state_transitions() {
        let mut detector = CodeBlockDetector::new();

        // Normal -> InlineCode
        detector.handle_backtick();
        assert_eq!(
            detector.evaluate_code_block_state(),
            StateTransition::Transition(CodeBlockState::InlineCode)
        );

        // InlineCode -> Normal
        detector.handle_backtick();
        assert_eq!(
            detector.evaluate_code_block_state(),
            StateTransition::Transition(CodeBlockState::Normal)
        );
    }

    #[test]
    fn test_triple_backtick_state_transitions() {
        let mut detector = CodeBlockDetector::new();

        // Normal -> CodeBlock
        for _ in 0..3 {
            detector.handle_backtick();
        }
        assert_eq!(
            detector.evaluate_code_block_state(),
            StateTransition::Transition(CodeBlockState::CodeBlock)
        );

        // CodeBlock -> Normal
        for _ in 0..3 {
            detector.handle_backtick();
        }
        assert_eq!(
            detector.evaluate_code_block_state(),
            StateTransition::Transition(CodeBlockState::Normal)
        );
    }

    #[test]
    fn test_invalid_backtick_counts() {
        let mut detector = CodeBlockDetector::new();

        // Two backticks should not trigger state change
        for _ in 0..2 {
            detector.handle_backtick();
        }
        assert_eq!(
            detector.evaluate_code_block_state(),
            StateTransition::NoTransition(2)
        );

        // Four backticks should not trigger state change
        for _ in 0..4 {
            detector.handle_backtick();
        }
        assert_eq!(
            detector.evaluate_code_block_state(),
            StateTransition::NoTransition(4)
        );
    }

    #[test]
    fn test_state_specific_transitions() {
        let mut detector = CodeBlockDetector::new();

        // Enter code block
        for _ in 0..3 {
            detector.handle_backtick();
        }
        assert_eq!(
            detector.evaluate_code_block_state(),
            StateTransition::Transition(CodeBlockState::CodeBlock)
        );

        // Single backtick in code block should not change state
        detector.handle_backtick();
        assert_eq!(
            detector.evaluate_code_block_state(),
            StateTransition::NoTransition(1)
        );

        // Exit code block
        for _ in 0..3 {
            detector.handle_backtick();
        }
        assert_eq!(
            detector.evaluate_code_block_state(),
            StateTransition::Transition(CodeBlockState::Normal)
        );

        // Enter inline code
        detector.handle_backtick();
        assert_eq!(
            detector.evaluate_code_block_state(),
            StateTransition::Transition(CodeBlockState::InlineCode)
        );

        // Triple backticks in inline code should not change state
        for _ in 0..3 {
            detector.handle_backtick();
        }
        assert_eq!(
            detector.evaluate_code_block_state(),
            StateTransition::NoTransition(3)
        );
    }
}
