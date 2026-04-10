#[derive(Debug, Clone, Copy)]
pub(crate) struct SingleLineLayout {
    pub(crate) font_size: u32,
    pub(crate) max_width: u32,
    pub(crate) letter_spacing: f32,
}

pub(crate) fn fit_single_line(
    text: &str,
    language: Option<&str>,
    base_font_size: u32,
    max_width: u32,
    letter_spacing: f32,
    min_font_size: u32,
) -> SingleLineLayout {
    if text.trim().is_empty() {
        return SingleLineLayout {
            font_size: base_font_size,
            max_width,
            letter_spacing,
        };
    }
    let estimated = estimate_text_width(text, language, base_font_size as f32, letter_spacing);
    let ratio = (max_width as f32 / estimated).min(1.0);
    let scaled_font = ((base_font_size as f32) * ratio).floor() as u32;
    SingleLineLayout {
        font_size: scaled_font.max(min_font_size).min(base_font_size),
        max_width,
        letter_spacing,
    }
}

pub(crate) fn render_single_line_text(
    x: u32,
    top: u32,
    font_family: &str,
    fill: &str,
    text: &str,
    language: Option<&str>,
    font_size: u32,
    max_width: u32,
    letter_spacing: f32,
) -> String {
    let estimated = estimate_text_width(text, language, font_size as f32, letter_spacing);
    let compression = if estimated > max_width as f32 {
        format!(
            " textLength=\"{}\" lengthAdjust=\"spacingAndGlyphs\"",
            max_width
        )
    } else {
        String::new()
    };
    let letter_spacing_attr = if letter_spacing.abs() > f32::EPSILON {
        format!(" letter-spacing=\"{}\"", letter_spacing)
    } else {
        String::new()
    };
    format!(
        "<text x=\"{}\" y=\"{}\" dominant-baseline=\"text-before-edge\" font-size=\"{}\" font-family=\"{}\" fill=\"{}\"{}{}>{}</text>",
        x,
        top,
        font_size,
        font_family,
        fill,
        letter_spacing_attr,
        compression,
        escape_xml(text)
    )
}

pub(crate) fn render_multiline_text(
    x: u32,
    top: u32,
    width: u32,
    height: u32,
    font_family: &str,
    fill: &str,
    text: &str,
    language: Option<&str>,
    base_font_size: u32,
    line_height: f32,
    letter_spacing: f32,
    min_font_size: u32,
) -> String {
    let text = text.trim_end();
    if text.is_empty() {
        return String::new();
    }

    let mut font_size = base_font_size;
    let mut lines = wrap_text(
        text,
        language,
        width as f32,
        font_size as f32,
        letter_spacing,
    );
    while font_size > min_font_size
        && total_text_height(lines.len(), font_size, line_height) > height as f32
    {
        font_size -= 1;
        lines = wrap_text(
            text,
            language,
            width as f32,
            font_size as f32,
            letter_spacing,
        );
    }

    let mut output = format!(
        "<text x=\"{}\" y=\"{}\" dominant-baseline=\"text-before-edge\" font-size=\"{}\" font-family=\"{}\" fill=\"{}\"",
        x, top, font_size, font_family, fill
    );
    if letter_spacing.abs() > f32::EPSILON {
        output.push_str(&format!(" letter-spacing=\"{}\"", letter_spacing));
    }
    output.push('>');
    for (index, line) in lines.iter().enumerate() {
        let dy = if index == 0 {
            0.0
        } else {
            font_size as f32 * line_height
        };
        output.push_str(&format!(
            "<tspan x=\"{}\" dy=\"{}\">{}</tspan>",
            x,
            dy,
            escape_xml(line)
        ));
    }
    output.push_str("</text>");
    output
}

fn wrap_text(
    text: &str,
    language: Option<&str>,
    max_width: f32,
    font_size: f32,
    letter_spacing: f32,
) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    for raw_line in text.replace("\r\n", "\n").split('\n') {
        if raw_line.is_empty() {
            if !current.is_empty() {
                lines.push(std::mem::take(&mut current));
            }
            lines.push(String::new());
            continue;
        }
        for token in tokenize_line(raw_line) {
            let candidate = if current.is_empty() {
                token.clone()
            } else {
                format!("{current}{token}")
            };
            let width = estimate_text_width(&candidate, language, font_size, letter_spacing);
            if !current.is_empty() && width > max_width {
                lines.push(std::mem::take(&mut current));
                if token.trim().is_empty() {
                    continue;
                }
                current.push_str(token.trim_start());
            } else {
                current.push_str(&token);
            }
        }
        if !current.is_empty() {
            lines.push(std::mem::take(&mut current));
        }
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

fn tokenize_line(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut word = String::new();
    for ch in text.chars() {
        if ch.is_ascii_whitespace() {
            if !word.is_empty() {
                tokens.push(std::mem::take(&mut word));
            }
            tokens.push(" ".to_string());
        } else if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '\'' | '/' | ':' | ',' | '.') {
            word.push(ch);
        } else {
            if !word.is_empty() {
                tokens.push(std::mem::take(&mut word));
            }
            tokens.push(ch.to_string());
        }
    }
    if !word.is_empty() {
        tokens.push(word);
    }
    tokens
}

fn total_text_height(line_count: usize, font_size: u32, line_height: f32) -> f32 {
    if line_count == 0 {
        0.0
    } else {
        font_size as f32 + (line_count.saturating_sub(1) as f32 * font_size as f32 * line_height)
    }
}

fn estimate_text_width(
    text: &str,
    language: Option<&str>,
    font_size: f32,
    letter_spacing: f32,
) -> f32 {
    let mut width = 0.0;
    let is_cjk_lang = matches!(language.unwrap_or("sc"), "sc" | "tc" | "jp");
    let char_count = text.chars().count();
    for ch in text.chars() {
        let factor = if ch.is_ascii_whitespace() {
            0.32
        } else if ch.is_ascii_digit() {
            0.58
        } else if ch.is_ascii_uppercase() {
            0.66
        } else if ch.is_ascii_lowercase() {
            0.54
        } else if ch.is_ascii_punctuation() {
            0.36
        } else if is_cjk_lang {
            1.0
        } else {
            0.85
        };
        width += font_size * factor;
    }
    if char_count > 1 {
        width += letter_spacing * (char_count.saturating_sub(1) as f32);
    }
    width
}

pub(crate) fn escape_xml(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
