use crate::config;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocBlockKind {
    Heading(u8),
    Paragraph,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocBlock {
    pub kind: DocBlockKind,
    /// Full preserved text for the folio panel, truncated on a character boundary.
    pub text: String,
    /// Shorter glyph preview, also character-safe.
    pub preview: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParseLimits {
    pub max_bytes: usize,
    pub max_blocks: usize,
    pub max_text_chars: usize,
}

impl Default for ParseLimits {
    fn default() -> Self {
        Self {
            max_bytes: config::DOCUMENT_MAX_BYTES,
            max_blocks: config::DOCUMENT_MAX_BLOCKS,
            max_text_chars: config::DOCUMENT_MAX_TEXT_CHARS,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseOutcome {
    pub blocks: Vec<DocBlock>,
    pub truncated: bool,
    pub lossy_utf8: bool,
}

/// Parses ATX/setext headings and blank-line paragraphs. Empty input yields one
/// synthetic paragraph so an empty file is still a visible page.
#[cfg_attr(not(test), allow(dead_code))]
pub fn parse_markdown_document(source: &str, limits: ParseLimits) -> ParseOutcome {
    parse_markdown_bytes(source.as_bytes(), limits, false)
}

pub fn parse_markdown_bytes(
    bytes: &[u8],
    limits: ParseLimits,
    already_lossy: bool,
) -> ParseOutcome {
    let truncated_bytes = bytes.len() > limits.max_bytes;
    let slice = if truncated_bytes {
        let mut end = limits.max_bytes;
        while end > 0 && end < bytes.len() && !is_char_boundary(bytes, end) {
            end -= 1;
        }
        &bytes[..end]
    } else {
        bytes
    };

    let (text, lossy_utf8) = match std::str::from_utf8(slice) {
        Ok(text) => (text.to_string(), already_lossy),
        Err(_) => (String::from_utf8_lossy(slice).into_owned(), true),
    };

    let mut blocks = Vec::new();
    let mut truncated = truncated_bytes;
    let mut paragraph = String::new();
    let lines: Vec<&str> = text.lines().collect();
    let mut index = 0;

    while index < lines.len() {
        if blocks.len() >= limits.max_blocks {
            truncated = true;
            break;
        }

        let line = lines[index];
        let trimmed = line.trim();

        if trimmed.is_empty() {
            flush_paragraph(&mut blocks, &mut paragraph, &limits);
            index += 1;
            continue;
        }

        if let Some((level, heading)) = parse_atx_heading(trimmed) {
            flush_paragraph(&mut blocks, &mut paragraph, &limits);
            push_block(&mut blocks, DocBlockKind::Heading(level), heading, &limits);
            index += 1;
            continue;
        }

        if let Some(level) = peek_setext_underline(lines.get(index + 1).copied()) {
            flush_paragraph(&mut blocks, &mut paragraph, &limits);
            push_block(&mut blocks, DocBlockKind::Heading(level), trimmed, &limits);
            index += 2;
            continue;
        }

        if !paragraph.is_empty() {
            paragraph.push(' ');
        }
        paragraph.push_str(trimmed);
        index += 1;
    }

    flush_paragraph(&mut blocks, &mut paragraph, &limits);

    if blocks.is_empty() {
        push_block(
            &mut blocks,
            DocBlockKind::Paragraph,
            "(empty document)",
            &limits,
        );
    }

    ParseOutcome {
        blocks,
        truncated,
        lossy_utf8,
    }
}

fn is_char_boundary(bytes: &[u8], index: usize) -> bool {
    bytes
        .get(index)
        .is_none_or(|byte| byte & 0b1100_0000 != 0b1000_0000)
}

fn parse_atx_heading(line: &str) -> Option<(u8, &str)> {
    let mut hashes = 0u8;
    let bytes = line.as_bytes();
    while hashes < 6 && bytes.get(hashes as usize) == Some(&b'#') {
        hashes += 1;
    }
    if hashes == 0 {
        return None;
    }
    let rest = line.get(hashes as usize..)?;
    let heading = rest.strip_prefix(' ')?;
    Some((hashes, heading.trim()))
}

fn peek_setext_underline(next: Option<&str>) -> Option<u8> {
    let line = next?.trim();
    if line.is_empty() {
        return None;
    }
    if line.chars().all(|ch| ch == '=') {
        Some(1)
    } else if line.chars().all(|ch| ch == '-') {
        Some(2)
    } else {
        None
    }
}

fn flush_paragraph(blocks: &mut Vec<DocBlock>, paragraph: &mut String, limits: &ParseLimits) {
    if paragraph.is_empty() || blocks.len() >= limits.max_blocks {
        paragraph.clear();
        return;
    }
    push_block(blocks, DocBlockKind::Paragraph, paragraph, limits);
    paragraph.clear();
}

fn push_block(blocks: &mut Vec<DocBlock>, kind: DocBlockKind, text: &str, limits: &ParseLimits) {
    let text = truncate_chars(text.trim(), limits.max_text_chars);
    if text.is_empty() {
        return;
    }
    let preview_limit = match kind {
        DocBlockKind::Heading(_) => config::DOCUMENT_HEADING_GLYPHS,
        DocBlockKind::Paragraph => config::DOCUMENT_PARAGRAPH_GLYPHS,
    };
    let preview = truncate_chars(&text, preview_limit);
    blocks.push(DocBlock {
        kind,
        text,
        preview,
    });
}

pub fn truncate_chars(text: &str, max_chars: usize) -> String {
    let count = text.chars().count();
    if count <= max_chars {
        return text.to_string();
    }
    text.chars().take(max_chars).collect()
}

#[cfg(test)]
mod tests {
    use super::{DocBlockKind, ParseLimits, parse_markdown_bytes, parse_markdown_document};

    fn limits() -> ParseLimits {
        ParseLimits {
            max_bytes: 1_024,
            max_blocks: 8,
            max_text_chars: 40,
        }
    }

    #[test]
    fn empty_input_synthesizes_a_visible_block() {
        let outcome = parse_markdown_document("   \n\n", limits());
        assert_eq!(outcome.blocks.len(), 1);
        assert_eq!(outcome.blocks[0].kind, DocBlockKind::Paragraph);
        assert_eq!(outcome.blocks[0].text, "(empty document)");
    }

    #[test]
    fn atx_and_setext_headings_and_paragraphs_parse() {
        let source = "# Title\n\nA paragraph.\n\nSubtitle\n---\n\nMore words.\n";
        let outcome = parse_markdown_document(source, limits());
        assert_eq!(
            outcome
                .blocks
                .iter()
                .map(|block| block.kind)
                .collect::<Vec<_>>(),
            vec![
                DocBlockKind::Heading(1),
                DocBlockKind::Paragraph,
                DocBlockKind::Heading(2),
                DocBlockKind::Paragraph,
            ]
        );
        assert_eq!(outcome.blocks[0].text, "Title");
    }

    #[test]
    fn hash_without_space_is_a_paragraph() {
        let outcome = parse_markdown_document("#NotAHeading\n", limits());
        assert_eq!(outcome.blocks[0].kind, DocBlockKind::Paragraph);
    }

    #[test]
    fn unicode_is_truncated_on_character_boundaries() {
        let outcome = parse_markdown_document("# 日本語タイトル\n\nééééééééééééééé", limits());
        assert!(!outcome.blocks[0].text.contains('\u{FFFD}'));
        assert!(outcome.blocks[1].text.chars().count() <= 40);
    }

    #[test]
    fn oversized_input_sets_truncated() {
        let source = "word\n\n".repeat(20);
        let outcome = parse_markdown_document(&source, limits());
        assert!(outcome.truncated);
        assert!(outcome.blocks.len() <= 8);
    }

    #[test]
    fn invalid_utf8_is_lossy_not_a_panic() {
        let outcome = parse_markdown_bytes(&[0xff, b'h', b'i'], limits(), false);
        assert!(outcome.lossy_utf8);
        assert!(!outcome.blocks.is_empty());
    }

    #[test]
    fn malformed_markdown_becomes_paragraphs() {
        let outcome = parse_markdown_document("```\nunterminated\n# still text", limits());
        assert!(outcome.blocks.iter().all(|block| matches!(
            block.kind,
            DocBlockKind::Paragraph | DocBlockKind::Heading(_)
        )));
    }
}
