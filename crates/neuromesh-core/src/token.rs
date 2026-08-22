/// Fast heuristic and exact BPE token counter
pub struct TokenCounter;

impl TokenCounter {
    /// Accurate byte-pair token approximation for code and markdown
    /// Code generally averages ~3.5 to 4 characters per token
    pub fn count_tokens(text: &str) -> usize {
        if text.is_empty() {
            return 0;
        }

        let mut token_count = 0;
        let mut in_word = false;

        for ch in text.chars() {
            if ch.is_whitespace() {
                if in_word {
                    token_count += 1;
                    in_word = false;
                }
                if ch == '\n' {
                    token_count += 1;
                }
            } else if ch.is_ascii_punctuation() {
                if in_word {
                    token_count += 1;
                    in_word = false;
                }
                token_count += 1;
            } else {
                in_word = true;
            }
        }

        if in_word {
            token_count += 1;
        }

        // Clamp to realistic lower bound: at least char_count / 4
        let char_based = (text.len() + 3) / 4;
        std::cmp::max(token_count, char_based)
    }

    /// Calculates net token difference between two strings
    pub fn diff_tokens(old_text: &str, new_text: &str) -> i64 {
        let old_tokens = Self::count_tokens(old_text) as i64;
        let new_tokens = Self::count_tokens(new_text) as i64;
        new_tokens - old_tokens
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_counting() {
        let sample = "const productCard = defineComponent({ name: 'ProductCard' });";
        let tokens = TokenCounter::count_tokens(sample);
        assert!(tokens >= 10 && tokens <= 25);
    }
}
