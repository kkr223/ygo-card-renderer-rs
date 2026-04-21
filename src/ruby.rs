/// Ruby/Furigana markup parser and layout constants.
///
/// Markup format: `[base(rt)]` e.g. `[魔(ま)][法(ほう)]カード`
/// Newlines are represented as explicit `RubyToken::Newline` tokens.

// ── Width strategy constants (mirrors compress-text.js) ──────────────────────

/// If rt_width / base_width < this threshold AND rt has >1 character, stretch via letter-spacing.
pub const RT_STRETCH_RATE: f32 = 0.9;

/// If base_width / rt_width < this threshold, apply scaleX = RT_COMPRESS_RATE + side padding.
pub const RT_COMPRESS_RATE: f32 = 0.6;

/// Maximum side padding (px, before line scale_x) added when rt is very wide.
pub const RUBY_PADDING_MAX: f32 = 5.0;

// ── Token ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum RubyToken {
    /// Ordinary text with no annotation.
    Plain(String),
    /// Annotated text: `base` drawn normally, `rt` drawn above it.
    Ruby { base: String, rt: String },
    /// Explicit newline (from `\n` in the source string).
    Newline,
}

impl RubyToken {
    /// The base / display text of this token (the part that occupies inline space).
    pub fn base_text(&self) -> &str {
        match self {
            RubyToken::Plain(s) => s.as_str(),
            RubyToken::Ruby { base, .. } => base.as_str(),
            RubyToken::Newline => "\n",
        }
    }

    /// The annotation text, or `None` for non-Ruby tokens.
    pub fn rt_text(&self) -> Option<&str> {
        match self {
            RubyToken::Ruby { rt, .. } => Some(rt.as_str()),
            _ => None,
        }
    }

    pub fn is_newline(&self) -> bool {
        matches!(self, RubyToken::Newline)
    }

    pub fn is_plain(&self) -> bool {
        matches!(self, RubyToken::Plain(_))
    }
}

// ── Parser ────────────────────────────────────────────────────────────────────

/// Parse a string that may contain `[base(rt)]` markup into a token list.
///
/// Unknown / malformed `[…]` sequences are left as-is inside a `Plain` token.
pub fn parse_ruby_text(text: &str) -> Vec<RubyToken> {
    let mut tokens: Vec<RubyToken> = Vec::new();
    let bytes = text.as_bytes();
    let len = text.len();
    let mut plain_start = 0usize;
    let mut i = 0usize;

    while i < len {
        match bytes[i] {
            b'\n' => {
                flush_plain(text, plain_start, i, &mut tokens);
                tokens.push(RubyToken::Newline);
                i += 1;
                plain_start = i;
            }
            b'[' => {
                if let Some((base, rt, consumed)) = parse_ruby_bracket(&text[i..]) {
                    flush_plain(text, plain_start, i, &mut tokens);
                    tokens.push(RubyToken::Ruby { base, rt });
                    i += consumed;
                    plain_start = i;
                } else {
                    i += 1;
                }
            }
            _ => {
                i += 1;
            }
        }
    }

    flush_plain(text, plain_start, len, &mut tokens);
    tokens
}

/// Return `true` when the text contains at least one valid `[base(rt)]` sequence.
pub fn contains_ruby_markup(text: &str) -> bool {
    // Quick structural check before full parse.
    let bytes = text.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] == b'[' {
            if parse_ruby_bracket(&text[i..]).is_some() {
                return true;
            }
        }
        i += 1;
    }
    false
}

/// Return the text with all ruby markup stripped (only base characters remain).
pub fn strip_ruby_markup(text: &str) -> String {
    let tokens = parse_ruby_text(text);
    let mut out = String::with_capacity(text.len());
    for token in tokens {
        match token {
            RubyToken::Plain(s) => out.push_str(&s),
            RubyToken::Ruby { base, .. } => out.push_str(&base),
            RubyToken::Newline => out.push('\n'),
        }
    }
    out
}

// ── Internal helpers ──────────────────────────────────────────────────────────

/// Attempt to parse `[base(rt)]` at the very beginning of `text`.
///
/// Returns `(base, rt, bytes_consumed)` on success, `None` otherwise.
pub(crate) fn parse_ruby_bracket(text: &str) -> Option<(String, String, usize)> {
    // Must start with '['.
    if !text.starts_with('[') {
        return None;
    }

    // Locate the '(' that separates base from rt.
    let open_paren = text.find('(')?;
    if open_paren == 1 {
        // Empty base "[("  – invalid.
        return None;
    }
    let base = text[1..open_paren].to_string();

    // Locate the closing ')' that ends the rt.
    let rest = &text[open_paren + 1..];
    let close_paren = rest.find(')')?;
    let rt = rest[..close_paren].to_string();

    // Must be followed by ']'.
    let after_close = &rest[close_paren + 1..];
    if !after_close.starts_with(']') {
        return None;
    }

    // bytes consumed = '[' + base + '(' + rt + ')' + ']'
    let consumed = 1 + base.len() + 1 + rt.len() + 1 + 1;
    Some((base, rt, consumed))
}

/// Flush `text[plain_start..plain_end]` as a `Plain` token (if non-empty).
fn flush_plain(text: &str, plain_start: usize, plain_end: usize, tokens: &mut Vec<RubyToken>) {
    if plain_start < plain_end {
        let s = text[plain_start..plain_end].to_string();
        if !s.is_empty() {
            tokens.push(RubyToken::Plain(s));
        }
    }
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_basic() {
        let tokens = parse_ruby_text("[魔(ま)][法(ほう)]カード");
        assert_eq!(tokens.len(), 3);
        match &tokens[0] {
            RubyToken::Ruby { base, rt } => {
                assert_eq!(base, "魔");
                assert_eq!(rt, "ま");
            }
            _ => panic!("expected Ruby"),
        }
        match &tokens[1] {
            RubyToken::Ruby { base, rt } => {
                assert_eq!(base, "法");
                assert_eq!(rt, "ほう");
            }
            _ => panic!("expected Ruby"),
        }
        match &tokens[2] {
            RubyToken::Plain(s) => assert_eq!(s, "カード"),
            _ => panic!("expected Plain"),
        }
    }

    #[test]
    fn test_parse_no_markup() {
        let tokens = parse_ruby_text("魔法カード");
        assert_eq!(tokens.len(), 1);
        assert!(tokens[0].is_plain());
        assert_eq!(tokens[0].base_text(), "魔法カード");
    }

    #[test]
    fn test_parse_newline() {
        let tokens = parse_ruby_text("[効(こう)]\n果");
        assert_eq!(tokens.len(), 3);
        assert!(tokens[1].is_newline());
        match &tokens[2] {
            RubyToken::Plain(s) => assert_eq!(s, "果"),
            _ => panic!("expected Plain"),
        }
    }

    #[test]
    fn test_contains_ruby() {
        assert!(contains_ruby_markup("[魔(ま)]"));
        assert!(!contains_ruby_markup("魔法カード"));
        assert!(!contains_ruby_markup("[broken"));
    }

    #[test]
    fn test_strip_ruby() {
        assert_eq!(strip_ruby_markup("[魔(ま)][法(ほう)]カード"), "魔法カード");
        assert_eq!(strip_ruby_markup("普通テキスト"), "普通テキスト");
        assert_eq!(strip_ruby_markup("[A(b)]\n[C(d)]"), "A\nC");
    }

    #[test]
    fn test_malformed_not_parsed() {
        // Missing ')' – should stay as plain text
        let tokens = parse_ruby_text("[魔(ま");
        assert_eq!(tokens.len(), 1);
        assert!(tokens[0].is_plain());
    }
}
