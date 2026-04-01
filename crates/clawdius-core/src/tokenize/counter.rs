//! Token Counter Implementation
//!
//! Provides multiple tokenization strategies for accurate token counting.

use std::collections::HashSet;

/// Tokenization strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TokenizerStrategy {
    /// Simple whitespace splitting (fast but inaccurate)
    Simple,
    /// Code-aware tokenization (handles programming syntax)
    Code,
    /// Natural language optimized (handles prose better)
    Natural,
    /// GPT-style BPE approximation
    #[default]
    BpeApproximation,
}

/// Counts tokens in text using the specified strategy.
///
/// # Arguments
///
/// * `text` - The text to count tokens in
/// * `strategy` - The tokenization strategy to use
///
/// # Returns
///
/// Estimated number of tokens.
#[must_use]
pub fn count_tokens(text: &str, strategy: TokenizerStrategy) -> usize {
    if text.is_empty() {
        return 0;
    }

    match strategy {
        TokenizerStrategy::Simple => count_simple(text),
        TokenizerStrategy::Code => count_code(text),
        TokenizerStrategy::Natural => count_natural(text),
        TokenizerStrategy::BpeApproximation => count_bpe_approximation(text),
    }
}

/// Simple whitespace-based tokenization.
fn count_simple(text: &str) -> usize {
    text.split_whitespace().count()
}

/// Code-aware tokenization.
///
/// Handles:
/// - Identifiers (camelCase, snake_case)
/// - Operators and punctuation
/// - String literals
/// - Comments
/// - Numbers
fn count_code(text: &str) -> usize {
    let mut count = 0;
    let mut chars = text.chars().peekable();
    let mut current_token = String::new();

    // Common operators and punctuation that are typically single tokens
    let single_char_tokens: HashSet<char> = ['{', '}', '(', ')', '[', ']', ';', ',', '.', ':']
        .into_iter()
        .collect();

    let operators: HashSet<&str> = [
        "==", "!=", "<=", ">=", "->", "=>", "::", "++", "--", "+=", "-=", "*=", "/=", "&&", "||",
        "??", "?.", "!!",
    ]
    .into_iter()
    .collect();

    while let Some(c) = chars.next() {
        // Whitespace - flush current token
        if c.is_whitespace() {
            if !current_token.is_empty() {
                count += count_identifier_tokens(&current_token);
                current_token.clear();
            }
            continue;
        }

        // Single character tokens
        if single_char_tokens.contains(&c) {
            if !current_token.is_empty() {
                count += count_identifier_tokens(&current_token);
                current_token.clear();
            }
            count += 1;
            continue;
        }

        // Check for two-character operators
        if let Some(&next) = chars.peek() {
            let two_char: String = format!("{c}{next}");
            if operators.contains(two_char.as_str()) {
                if !current_token.is_empty() {
                    count += count_identifier_tokens(&current_token);
                    current_token.clear();
                }
                count += 1;
                chars.next(); // consume the second character
                continue;
            }
        }

        // String literals
        if c == '"' || c == '\'' {
            if !current_token.is_empty() {
                count += count_identifier_tokens(&current_token);
                current_token.clear();
            }
            let quote = c;
            let mut string_content = String::new();
            loop {
                match chars.next() {
                    Some('\\') => {
                        // Escape sequence
                        string_content.push('\\');
                        if let Some(escaped) = chars.next() {
                            string_content.push(escaped);
                        }
                    },
                    Some(q) if q == quote => {
                        break;
                    },
                    Some(q) => string_content.push(q),
                    None => break,
                }
            }
            // String content tokens + opening/closing quotes
            count += count_string_tokens(&string_content) + 2;
            continue;
        }

        // Numbers
        if c.is_ascii_digit() {
            if !current_token.is_empty() {
                count += count_identifier_tokens(&current_token);
                current_token.clear();
            }
            let mut num_str = String::from(c);
            while let Some(&next) = chars.peek() {
                if next.is_ascii_digit() || next == '.' || next == '_' {
                    num_str.push(next);
                    chars.next();
                } else {
                    break;
                }
            }
            count += 1; // Numbers are typically single tokens
            continue;
        }

        // Accumulate for identifiers
        current_token.push(c);
    }

    // Flush remaining token
    if !current_token.is_empty() {
        count += count_identifier_tokens(&current_token);
    }

    count.max(1)
}

/// Count tokens in an identifier (handles camelCase, snake_case).
fn count_identifier_tokens(ident: &str) -> usize {
    if ident.is_empty() {
        return 0;
    }

    // Check for common keywords (single tokens)
    let keywords: HashSet<&str> = [
        "fn",
        "let",
        "const",
        "var",
        "if",
        "else",
        "for",
        "while",
        "loop",
        "match",
        "return",
        "break",
        "continue",
        "struct",
        "enum",
        "impl",
        "trait",
        "pub",
        "mod",
        "use",
        "self",
        "Self",
        "true",
        "false",
        "None",
        "Some",
        "Ok",
        "Err",
        "async",
        "await",
        "move",
        "ref",
        "mut",
        "where",
        "type",
        "static",
        "extern",
        "crate",
        "super",
        "dyn",
        "unsafe",
        "function",
        "class",
        "interface",
        "extends",
        "implements",
        "import",
        "export",
        "default",
        "from",
        "as",
        "new",
        "this",
        "super",
        "throw",
        "try",
        "catch",
        "finally",
        "typeof",
        "instanceof",
        "void",
        "null",
        "undefined",
        "const",
        "let",
        "var",
        "def",
        "end",
        "do",
    ]
    .into_iter()
    .collect();

    if keywords.contains(ident) {
        return 1;
    }

    let mut count = 0;
    let mut prev_is_upper = false;
    let mut prev_is_underscore = false;

    for c in ident.chars() {
        if c == '_' {
            if !prev_is_underscore {
                count += 1;
            }
            prev_is_underscore = true;
            prev_is_upper = false;
        } else if c.is_uppercase() {
            // CamelCase boundary
            if !prev_is_upper {
                count += 1;
            }
            prev_is_upper = true;
            prev_is_underscore = false;
        } else if c.is_lowercase() {
            prev_is_upper = false;
            prev_is_underscore = false;
        } else {
            // Non-alphabetic character
            count += 1;
            prev_is_upper = false;
            prev_is_underscore = false;
        }
    }

    // If no boundaries found, count as single token
    count.max(1)
}

/// Count tokens in a string literal.
fn count_string_tokens(s: &str) -> usize {
    // For short strings, count as 1-2 tokens
    if s.len() <= 10 {
        return 1;
    }

    // For longer strings, estimate based on word count
    let words = s.split_whitespace().count();
    words.max(2)
}

/// Natural language tokenization.
fn count_natural(text: &str) -> usize {
    let mut count = 0;

    // Split by whitespace
    for word in text.split_whitespace() {
        // Punctuation often becomes separate tokens
        let word_clean: String = word
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '\'' || *c == '-')
            .collect();

        if !word_clean.is_empty() {
            count += 1;

            // Long words may be split
            if word_clean.len() > 15 {
                count += (word_clean.len() - 10) / 5;
            }
        }

        // Count trailing punctuation
        let trailing_punct: usize = word
            .chars()
            .rev()
            .take_while(|c| !c.is_alphanumeric())
            .count();
        if trailing_punct > 0 && word_clean.is_empty() {
            count += 1;
        }
    }

    count.max(1)
}

/// BPE-style approximation (GPT tokenization).
///
/// This is an approximation based on observed patterns:
/// - ~4 characters per token for English text
/// - More tokens for code (more punctuation, symbols)
/// - Fewer tokens for repetitive content
fn count_bpe_approximation(text: &str) -> usize {
    if text.is_empty() {
        return 0;
    }

    // Count different character types
    let mut alpha_count = 0;
    let mut digit_count = 0;
    let mut punct_count = 0;
    let mut space_count = 0;
    let mut other_count = 0;

    for c in text.chars() {
        match c {
            'a'..='z' | 'A'..='Z' => alpha_count += 1,
            '0'..='9' => digit_count += 1,
            c if c.is_whitespace() => space_count += 1,
            c if c.is_ascii_punctuation() => punct_count += 1,
            _ => other_count += 1,
        }
    }

    // Base estimate: ~4 characters per token
    let total_chars = alpha_count + digit_count + punct_count + other_count;
    let base_tokens = (total_chars as f64 / 4.0).ceil() as usize;

    // Punctuation often adds extra tokens
    let punct_tokens = (punct_count as f64 / 6.0).ceil() as usize;

    // Spaces don't add tokens but affect boundaries
    let _ = space_count;

    // Code tends to have more tokens
    let code_factor = if punct_count > total_chars / 10 {
        1.1
    } else {
        1.0
    };

    let estimate = ((base_tokens + punct_tokens) as f64 * code_factor).ceil() as usize;

    estimate.max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple() {
        let text = "Hello world this is a test";
        assert_eq!(count_simple(text), 6);
    }

    #[test]
    fn test_code_simple() {
        let code = "fn main() {}";
        assert!(count_code(code) >= 5);
    }

    #[test]
    fn test_code_identifier() {
        // camelCase should split
        let ident = "myVariableName";
        assert!(count_identifier_tokens(ident) >= 2);

        // snake_case should split
        let ident = "my_variable_name";
        assert!(count_identifier_tokens(ident) >= 2);

        // single word
        let ident = "variable";
        assert_eq!(count_identifier_tokens(ident), 1);
    }

    #[test]
    fn test_bpe_approximation() {
        let text = "Hello, world! This is a test.";
        let tokens = count_bpe_approximation(text);
        assert!(tokens >= 5);
    }

    #[test]
    fn test_empty() {
        assert_eq!(count_tokens("", TokenizerStrategy::Simple), 0);
        assert_eq!(count_tokens("", TokenizerStrategy::Code), 0);
        assert_eq!(count_tokens("", TokenizerStrategy::Natural), 0);
        assert_eq!(count_tokens("", TokenizerStrategy::BpeApproximation), 0);
    }

    #[test]
    fn test_code_string_literal() {
        let code = r#"let s = "hello world";"#;
        let tokens = count_code(code);
        assert!(tokens >= 5);
    }

    #[test]
    fn test_code_operators() {
        let code = "if x == y && z != w {";
        let tokens = count_code(code);
        assert!(tokens >= 8);
    }

    #[test]
    fn test_natural() {
        let text = "The quick brown fox jumps over the lazy dog.";
        let tokens = count_natural(text);
        assert!(tokens >= 9);
    }
}
