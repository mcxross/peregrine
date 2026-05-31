use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use pulldown_cmark::{CowStr, Event, Options, Parser, html};
use serde::Serialize;
use std::{fs, path::Path, sync::OnceLock};
use syntect::{
    easy::HighlightLines,
    highlighting::{Style, Theme, ThemeSet},
    html::{IncludeBackground, styled_line_to_highlighted_html},
    parsing::SyntaxSet,
    util::LinesWithEndings,
};

const MAX_PREVIEW_BYTES: u64 = 25 * 1024 * 1024;
const SCAN_BYTES: usize = 16 * 1024;
const MAX_CONTROL_CHARACTER_RATIO: f32 = 0.05;

static SYNTAX_SET: OnceLock<SyntaxSet> = OnceLock::new();
static THEME_SET: OnceLock<ThemeSet> = OnceLock::new();

#[derive(Serialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum FilePreview {
    #[serde(rename = "text")]
    Text {
        path: String,
        language: String,
        source: String,
        #[serde(rename = "highlightedHtml")]
        highlighted_html: String,
    },
    #[serde(rename = "markdown")]
    Markdown {
        path: String,
        source: String,
        html: String,
    },
    #[serde(rename = "image")]
    Image {
        path: String,
        mime: String,
        #[serde(rename = "dataUrl")]
        data_url: String,
    },
    #[serde(rename = "video")]
    Video {
        path: String,
        mime: String,
        #[serde(rename = "dataUrl")]
        data_url: String,
    },
    #[serde(rename = "unsupported")]
    Unsupported {
        path: String,
        reason: String,
        size: u64,
    },
}

pub fn build_file_preview(
    file_path: &Path,
    relative_path: String,
    include_highlighted_html: bool,
) -> Result<FilePreview, String> {
    let metadata = fs::metadata(file_path)
        .map_err(|error| format!("Could not inspect {}: {error}", file_path.display()))?;

    if metadata.len() > MAX_PREVIEW_BYTES {
        return Ok(unsupported(
            relative_path,
            "File is larger than the preview limit.",
            metadata.len(),
        ));
    }

    let bytes = fs::read(file_path)
        .map_err(|error| format!("Could not read {}: {error}", file_path.display()))?;

    match classify_file(file_path, &bytes) {
        FileClass::Image(mime) => Ok(media_preview(relative_path, mime, bytes, MediaKind::Image)),
        FileClass::Video(mime) => Ok(media_preview(relative_path, mime, bytes, MediaKind::Video)),
        FileClass::Markdown => {
            let source = decode_text(&bytes);

            Ok(FilePreview::Markdown {
                path: relative_path,
                html: render_markdown(&source),
                source,
            })
        }
        FileClass::Text(syntax) => {
            let source = decode_text(&bytes);
            let highlighted_html = if include_highlighted_html {
                render_source(file_path, &source, syntax)
            } else {
                String::new()
            };

            Ok(FilePreview::Text {
                path: relative_path,
                language: syntax.language_name().to_string(),
                source,
                highlighted_html,
            })
        }
        FileClass::Binary => Ok(unsupported(
            relative_path,
            "Binary file preview is not supported for this file type.",
            metadata.len(),
        )),
    }
}

fn classify_file(file_path: &Path, bytes: &[u8]) -> FileClass {
    if let Some(mime) = image_mime(file_path) {
        return FileClass::Image(mime);
    }

    if let Some(mime) = video_mime(file_path) {
        return FileClass::Video(mime);
    }

    if !is_probably_text(bytes) {
        return FileClass::Binary;
    }

    if is_markdown(file_path) {
        return FileClass::Markdown;
    }

    FileClass::Text(syntax_for_file(file_path))
}

enum FileClass {
    Image(&'static str),
    Video(&'static str),
    Markdown,
    Text(SyntaxKind),
    Binary,
}

enum MediaKind {
    Image,
    Video,
}

fn media_preview(
    relative_path: String,
    mime: &'static str,
    bytes: Vec<u8>,
    kind: MediaKind,
) -> FilePreview {
    let data_url = format!("data:{mime};base64,{}", BASE64_STANDARD.encode(bytes));

    match kind {
        MediaKind::Image => FilePreview::Image {
            path: relative_path,
            mime: mime.to_string(),
            data_url,
        },
        MediaKind::Video => FilePreview::Video {
            path: relative_path,
            mime: mime.to_string(),
            data_url,
        },
    }
}

fn unsupported(relative_path: String, reason: &str, size: u64) -> FilePreview {
    FilePreview::Unsupported {
        path: relative_path,
        reason: reason.to_string(),
        size,
    }
}

fn image_mime(file_path: &Path) -> Option<&'static str> {
    match lowercase_extension(file_path).as_deref() {
        Some("apng") => Some("image/apng"),
        Some("avif") => Some("image/avif"),
        Some("gif") => Some("image/gif"),
        Some("jpg" | "jpeg") => Some("image/jpeg"),
        Some("png") => Some("image/png"),
        Some("svg") => Some("image/svg+xml"),
        Some("webp") => Some("image/webp"),
        _ => None,
    }
}

fn video_mime(file_path: &Path) -> Option<&'static str> {
    match lowercase_extension(file_path).as_deref() {
        Some("mp4" | "m4v") => Some("video/mp4"),
        Some("mov") => Some("video/quicktime"),
        Some("ogg" | "ogv") => Some("video/ogg"),
        Some("webm") => Some("video/webm"),
        _ => None,
    }
}

fn is_markdown(file_path: &Path) -> bool {
    matches!(
        lowercase_extension(file_path).as_deref(),
        Some("md" | "markdown" | "mdown" | "mkd")
    )
}

fn lowercase_extension(file_path: &Path) -> Option<String> {
    file_path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.to_ascii_lowercase())
}

fn is_probably_text(bytes: &[u8]) -> bool {
    if bytes.is_empty() {
        return true;
    }

    let sample = &bytes[..bytes.len().min(SCAN_BYTES)];

    if sample.contains(&0) {
        return false;
    }

    let control_count = sample
        .iter()
        .filter(|byte| byte.is_ascii_control() && !matches!(byte, b'\n' | b'\r' | b'\t'))
        .count();

    (control_count as f32 / sample.len() as f32) <= MAX_CONTROL_CHARACTER_RATIO
}

fn decode_text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

#[derive(Clone, Copy)]
enum SyntaxKind {
    Move,
    Syntect(&'static str),
    Plain,
}

impl SyntaxKind {
    fn language_name(self) -> &'static str {
        match self {
            SyntaxKind::Move => "Move",
            SyntaxKind::Syntect(language) => language,
            SyntaxKind::Plain => "Plain text",
        }
    }
}

fn syntax_for_file(file_path: &Path) -> SyntaxKind {
    match lowercase_extension(file_path).as_deref() {
        Some("move") => SyntaxKind::Move,
        Some("bash" | "sh" | "zsh") => SyntaxKind::Syntect("Shell"),
        Some("css") => SyntaxKind::Syntect("CSS"),
        Some("html" | "htm") => SyntaxKind::Syntect("HTML"),
        Some("js") => SyntaxKind::Syntect("JavaScript"),
        Some("jsx") => SyntaxKind::Syntect("JSX"),
        Some("json") => SyntaxKind::Syntect("JSON"),
        Some("rs") => SyntaxKind::Syntect("Rust"),
        Some("toml") => SyntaxKind::Syntect("TOML"),
        Some("ts") => SyntaxKind::Syntect("TypeScript"),
        Some("tsx") => SyntaxKind::Syntect("TSX"),
        Some("yaml" | "yml") => SyntaxKind::Syntect("YAML"),
        _ => SyntaxKind::Plain,
    }
}

fn render_source(file_path: &Path, source: &str, syntax: SyntaxKind) -> String {
    match syntax {
        SyntaxKind::Move => highlight_move_source(source),
        SyntaxKind::Syntect(_) => highlight_source(file_path, source),
        SyntaxKind::Plain => escape_html(source),
    }
}

fn highlight_source(file_path: &Path, source: &str) -> String {
    let syntax_set = SYNTAX_SET.get_or_init(SyntaxSet::load_defaults_newlines);
    let theme_set = THEME_SET.get_or_init(ThemeSet::load_defaults);
    let syntax = syntax_set
        .find_syntax_for_file(file_path)
        .ok()
        .flatten()
        .unwrap_or_else(|| syntax_set.find_syntax_plain_text());
    let Some(theme) = source_theme(theme_set) else {
        return escape_html(source);
    };
    let mut highlighter = HighlightLines::new(syntax, theme);
    let mut rendered = String::new();

    for line in LinesWithEndings::from(source) {
        let Ok(ranges) = highlighter.highlight_line(line, syntax_set) else {
            return escape_html(source);
        };
        let Ok(highlighted) = styled_line_to_highlighted_html(
            &ranges
                .iter()
                .map(|(style, text)| (*style, *text))
                .collect::<Vec<(Style, &str)>>(),
            IncludeBackground::No,
        ) else {
            return escape_html(source);
        };

        rendered.push_str(&highlighted);
    }

    rendered
}

fn source_theme(theme_set: &ThemeSet) -> Option<&Theme> {
    theme_set
        .themes
        .get("base16-ocean.dark")
        .or_else(|| theme_set.themes.values().next())
}

fn highlight_move_source(source: &str) -> String {
    const KEYWORDS: &[&str] = &[
        "abort", "acquires", "as", "break", "const", "continue", "copy", "else", "entry", "false",
        "friend", "fun", "has", "if", "let", "module", "move", "mut", "native", "public", "return",
        "script", "spec", "struct", "true", "use", "while",
    ];

    let mut rendered = String::new();

    for line in LinesWithEndings::from(source) {
        let mut token = String::new();

        for character in line.chars() {
            if character.is_ascii_alphanumeric() || character == '_' {
                token.push(character);
                continue;
            }

            push_move_token(&mut rendered, &token, KEYWORDS);
            token.clear();
            rendered.push_str(&escape_html_char(character));
        }

        push_move_token(&mut rendered, &token, KEYWORDS);
    }

    rendered
}

fn push_move_token(rendered: &mut String, token: &str, keywords: &[&str]) {
    if token.is_empty() {
        return;
    }

    if keywords.contains(&token) {
        rendered.push_str("<span style=\"color:#c678dd;font-weight:600\">");
        rendered.push_str(token);
        rendered.push_str("</span>");
    } else if token.chars().all(|character| character.is_ascii_digit()) {
        rendered.push_str("<span style=\"color:#d19a66\">");
        rendered.push_str(token);
        rendered.push_str("</span>");
    } else {
        rendered.push_str(&escape_html(token));
    }
}

fn render_markdown(source: &str) -> String {
    let parser = Parser::new_ext(source, Options::all()).map(|event| match event {
        Event::Html(html) | Event::InlineHtml(html) => {
            Event::Text(CowStr::Boxed(html.into_string().into_boxed_str()))
        }
        event => event,
    });
    let mut rendered = String::new();

    html::push_html(&mut rendered, parser);
    rendered
}

fn escape_html(value: &str) -> String {
    value
        .chars()
        .map(escape_html_char)
        .collect::<Vec<_>>()
        .join("")
}

fn escape_html_char(character: char) -> String {
    match character {
        '&' => "&amp;".to_string(),
        '<' => "&lt;".to_string(),
        '>' => "&gt;".to_string(),
        '"' => "&quot;".to_string(),
        '\'' => "&#39;".to_string(),
        _ => character.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn text_preview_can_include_highlighted_html() {
        let directory = tempdir().expect("tempdir");
        let file_path = directory.path().join("module.move");
        fs::write(&file_path, "module 0x1::example { fun demo() {} }\n").expect("write source");

        let preview =
            build_file_preview(&file_path, "module.move".to_string(), true).expect("preview");

        let FilePreview::Text {
            highlighted_html,
            language,
            source,
            ..
        } = preview
        else {
            panic!("expected text preview");
        };

        assert_eq!(language, "Move");
        assert_eq!(source, "module 0x1::example { fun demo() {} }\n");
        assert!(highlighted_html.contains("<span"));
        assert!(highlighted_html.contains("module"));
    }

    #[test]
    fn text_preview_can_skip_highlighted_html() {
        let directory = tempdir().expect("tempdir");
        let file_path = directory.path().join("module.move");
        fs::write(&file_path, "module 0x1::example { fun demo() {} }\n").expect("write source");

        let preview =
            build_file_preview(&file_path, "module.move".to_string(), false).expect("preview");

        let FilePreview::Text {
            highlighted_html,
            language,
            source,
            ..
        } = preview
        else {
            panic!("expected text preview");
        };

        assert_eq!(language, "Move");
        assert_eq!(source, "module 0x1::example { fun demo() {} }\n");
        assert_eq!(highlighted_html, "");
    }
}
