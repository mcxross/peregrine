use std::collections::BTreeMap;

use peregrine_types::analysis::{
    AnalysisContext, Finding, ParsedFunction, ParsedModule, RuleConfigProperty,
    RuleConfigValueKind, RuleMetadata, Severity, SourceFile, Span,
};

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Token {
    pub text: String,
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone)]
pub struct DeclaredItem {
    pub name: String,
    pub kind: DeclaredItemKind,
    pub file: String,
    pub span: Span,
    pub start_offset: usize,
    pub end_offset: usize,
    pub is_test_only: bool,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum DeclaredItemKind {
    Const,
    Enum,
    Struct,
}

pub fn finding(
    ruleset_id: &str,
    rule_id: &str,
    severity: Severity,
    message: impl Into<String>,
    file: impl Into<String>,
    span: Option<Span>,
) -> Finding {
    Finding {
        rule_id: rule_id.to_string(),
        ruleset_id: ruleset_id.to_string(),
        severity,
        message: message.into(),
        file: file.into(),
        span,
        metric: None,
    }
}

pub fn rule_metadata(
    id: &str,
    name: &str,
    description: &str,
    default_severity: Severity,
) -> RuleMetadata {
    RuleMetadata {
        id: id.to_string(),
        name: name.to_string(),
        description: description.to_string(),
        active: true,
        default_severity,
        configured_severity: None,
        config_schema: vec![RuleConfigProperty {
            key: "severity".to_string(),
            value_kind: RuleConfigValueKind::Severity,
            description: "Finding severity override.".to_string(),
            default_value: None,
        }],
    }
}

pub fn all_functions(context: &AnalysisContext) -> Vec<(&ParsedModule, &ParsedFunction)> {
    context
        .modules
        .iter()
        .flat_map(|module| {
            module
                .functions
                .iter()
                .map(move |function| (module, function))
        })
        .collect()
}

pub fn function_target(module: &ParsedModule, function: &ParsedFunction) -> String {
    format!("{}::{}", module.name, function.name)
}

pub fn sanitize_source(source: &str) -> String {
    #[derive(Clone, Copy)]
    enum State {
        Normal,
        LineComment,
        BlockComment,
        String { escaped: bool },
    }

    let bytes = source.as_bytes();
    let mut sanitized = Vec::with_capacity(bytes.len());
    let mut state = State::Normal;
    let mut index = 0_usize;

    while index < bytes.len() {
        let byte = bytes[index];
        let next = bytes.get(index + 1).copied();

        match state {
            State::Normal if byte == b'/' && next == Some(b'/') => {
                sanitized.extend_from_slice(b"  ");
                state = State::LineComment;
                index += 2;
            }
            State::Normal if byte == b'/' && next == Some(b'*') => {
                sanitized.extend_from_slice(b"  ");
                state = State::BlockComment;
                index += 2;
            }
            State::Normal if byte == b'"' => {
                sanitized.push(byte);
                state = State::String { escaped: false };
                index += 1;
            }
            State::Normal if byte == b'b' && next == Some(b'"') => {
                sanitized.extend_from_slice(b"  ");
                state = State::String { escaped: false };
                index += 2;
            }
            State::Normal => {
                sanitized.push(byte);
                index += 1;
            }
            State::LineComment if byte == b'\n' => {
                sanitized.push(byte);
                state = State::Normal;
                index += 1;
            }
            State::LineComment => {
                sanitized.push(b' ');
                index += 1;
            }
            State::BlockComment if byte == b'*' && next == Some(b'/') => {
                sanitized.extend_from_slice(b"  ");
                state = State::Normal;
                index += 2;
            }
            State::BlockComment => {
                sanitized.push(if byte == b'\n' { b'\n' } else { b' ' });
                index += 1;
            }
            State::String { escaped: true } => {
                sanitized.push(if byte == b'\n' { b'\n' } else { b' ' });
                state = State::String { escaped: false };
                index += 1;
            }
            State::String { escaped: false } if byte == b'\\' => {
                sanitized.push(b' ');
                state = State::String { escaped: true };
                index += 1;
            }
            State::String { escaped: false } if byte == b'"' => {
                sanitized.push(byte);
                state = State::Normal;
                index += 1;
            }
            State::String { escaped: false } => {
                sanitized.push(if byte == b'\n' { b'\n' } else { b' ' });
                index += 1;
            }
        }
    }

    String::from_utf8(sanitized).unwrap_or_else(|_| source.to_string())
}

pub fn tokenize(source: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut index = 0_usize;

    while index < source.len() {
        let Some(character) = source[index..].chars().next() else {
            break;
        };

        if character.is_whitespace() {
            index += character.len_utf8();
            continue;
        }

        if is_identifier_start(character) {
            let start = index;
            index += character.len_utf8();
            while index < source.len() {
                let Some(next) = source[index..].chars().next() else {
                    break;
                };
                if !is_identifier_continue(next) {
                    break;
                }
                index += next.len_utf8();
            }
            tokens.push(Token {
                text: source[start..index].to_string(),
                start,
                end: index,
            });
            continue;
        }

        if character.is_ascii_digit() {
            let start = index;
            index += character.len_utf8();
            while index < source.len() {
                let Some(next) = source[index..].chars().next() else {
                    break;
                };
                if !(next.is_ascii_alphanumeric() || next == '_') {
                    break;
                }
                index += next.len_utf8();
            }
            tokens.push(Token {
                text: source[start..index].to_string(),
                start,
                end: index,
            });
            continue;
        }

        let rest = &source[index..];
        let two = ["::", "==", "!=", "<=", ">=", "&&", "||", "=>"];
        if let Some(operator) = two.iter().find(|operator| rest.starts_with(**operator)) {
            tokens.push(Token {
                text: (*operator).to_string(),
                start: index,
                end: index + operator.len(),
            });
            index += operator.len();
            continue;
        }

        tokens.push(Token {
            text: character.to_string(),
            start: index,
            end: index + character.len_utf8(),
        });
        index += character.len_utf8();
    }

    tokens
}

pub fn line_number_at(source: &str, offset: usize) -> usize {
    source
        .as_bytes()
        .iter()
        .take(offset.min(source.len()))
        .filter(|byte| **byte == b'\n')
        .count()
        + 1
}

pub fn collect_declarations(context: &AnalysisContext) -> Vec<DeclaredItem> {
    context
        .source_files
        .iter()
        .flat_map(collect_file_declarations)
        .collect()
}

pub fn token_is_identifier(text: &str) -> bool {
    let mut chars = text.chars();
    chars.next().is_some_and(is_identifier_start) && chars.all(is_identifier_continue)
}

pub fn primitive_type_after_cast(tokens: &[Token], cast_index: usize) -> Option<&str> {
    tokens
        .get(cast_index + 1)
        .and_then(|token| primitive_numeric_type(token.text.as_str()))
}

pub fn primitive_numeric_type(text: &str) -> Option<&str> {
    match text {
        "u8" | "u16" | "u32" | "u64" | "u128" | "u256" => Some(text),
        _ => None,
    }
}

pub fn function_local_types(function: &ParsedFunction) -> BTreeMap<String, String> {
    let mut types = BTreeMap::new();
    collect_signature_parameter_types(&function.signature, &mut types);
    collect_let_annotation_types(&function.body, &mut types);
    types
}

pub fn function_has_return_value(function: &ParsedFunction) -> bool {
    let signature = function.signature.as_str();
    let Some(params_end) = matching_paren(signature, signature.find('(').unwrap_or(0)) else {
        return false;
    };
    let rest = signature[params_end + 1..].trim();
    let Some(return_type) = rest.strip_prefix(':') else {
        return false;
    };
    !return_type.trim().is_empty() && return_type.trim() != "()"
}

pub fn call_name_at(tokens: &[Token], index: usize) -> Option<(&str, usize)> {
    let token = tokens.get(index)?;
    if !token_is_identifier(&token.text) {
        return None;
    }
    if matches!(
        token.text.as_str(),
        "if" | "while" | "loop" | "fun" | "struct" | "enum" | "const" | "return" | "abort"
    ) {
        return None;
    }

    let mut cursor = index + 1;
    if tokens
        .get(cursor)
        .is_some_and(|candidate| candidate.text == "<")
    {
        cursor = skip_angle_group(tokens, cursor)?;
    }

    if tokens
        .get(cursor)
        .is_some_and(|candidate| candidate.text == "(")
    {
        Some((token.text.as_str(), cursor))
    } else {
        None
    }
}

pub fn is_function_declaration(tokens: &[Token], index: usize) -> bool {
    previous_significant(tokens, index).is_some_and(|previous| previous.text == "fun")
}

pub fn qualified_call_module<'a>(tokens: &'a [Token], index: usize) -> Option<&'a str> {
    if index >= 2 && tokens[index - 1].text == "::" && token_is_identifier(&tokens[index - 2].text)
    {
        Some(tokens[index - 2].text.as_str())
    } else {
        None
    }
}

pub fn find_matching_token(
    tokens: &[Token],
    open_index: usize,
    open: &str,
    close: &str,
) -> Option<usize> {
    let mut depth = 0_i32;

    for (index, token) in tokens.iter().enumerate().skip(open_index) {
        if token.text == open {
            depth += 1;
        } else if token.text == close {
            depth -= 1;
            if depth == 0 {
                return Some(index);
            }
        }
    }

    None
}

pub fn token_range_contains(tokens: &[Token], start: usize, end: usize, text: &str) -> bool {
    tokens
        .get(start..=end)
        .unwrap_or_default()
        .iter()
        .any(|token| token.text == text)
}

pub fn token_range_contains_call(tokens: &[Token], start: usize, end: usize, name: &str) -> bool {
    let Some(slice) = tokens.get(start..=end) else {
        return false;
    };

    slice
        .iter()
        .enumerate()
        .any(|(offset, token)| token.text == name && call_name_at(slice, offset).is_some())
}

pub fn token_line_span(source: &str, token: &Token, base_span: Option<Span>) -> Option<Span> {
    let line = line_number_at(source, token.start);
    Some(match base_span {
        Some(base) => Span {
            start_line: base.start_line + line.saturating_sub(1),
            end_line: base.start_line + line.saturating_sub(1),
        },
        None => Span {
            start_line: line,
            end_line: line,
        },
    })
}

pub fn has_test_attribute_near(source: &str, start: usize) -> bool {
    let window_start = source[..start.min(source.len())]
        .rfind(|character| matches!(character, '}' | ';'))
        .map(|index| index + 1)
        .unwrap_or(0);
    let prefix = &source[window_start..start.min(source.len())];

    prefix.contains("#[test")
        || prefix.contains("test_only")
        || prefix.contains("mode(test)")
        || prefix.contains("mode = test")
}

pub fn name_referenced_outside_declaration(context: &AnalysisContext, item: &DeclaredItem) -> bool {
    for file in &context.source_files {
        let sanitized = sanitize_source(&file.contents);
        for token in tokenize(&sanitized) {
            if token.text != item.name {
                continue;
            }
            if file.path == item.file
                && token.start >= item.start_offset
                && token.end <= item.end_offset
            {
                continue;
            }
            return true;
        }
    }

    false
}

pub fn called_by_function(
    caller_module: &ParsedModule,
    caller: &ParsedFunction,
    target_module: &str,
    target_name: &str,
) -> bool {
    let sanitized = sanitize_source(&caller.body);
    let tokens = tokenize(&sanitized);

    for (index, token) in tokens.iter().enumerate() {
        if token.text != target_name || is_function_declaration(&tokens, index) {
            continue;
        }
        let Some((_, _open_index)) = call_name_at(&tokens, index) else {
            continue;
        };
        let qualifier = qualified_call_module(&tokens, index);
        if qualifier.is_some_and(|module| module == target_module)
            || (qualifier.is_none() && caller_module.name == target_module)
        {
            return true;
        }
    }

    false
}

pub fn test_like_function(function: &ParsedFunction) -> bool {
    function.body.contains("#[test")
        || function.body.contains("test_only")
        || function.body.contains("mode(test)")
        || function.body.contains("mode = test")
}

fn collect_file_declarations(source_file: &SourceFile) -> Vec<DeclaredItem> {
    let sanitized = sanitize_source(&source_file.contents);
    let tokens = tokenize(&sanitized);
    let mut declarations = Vec::new();

    for (index, token) in tokens.iter().enumerate() {
        let kind = match token.text.as_str() {
            "const" => DeclaredItemKind::Const,
            "struct" => DeclaredItemKind::Struct,
            "enum" => DeclaredItemKind::Enum,
            _ => continue,
        };
        let Some(name) = tokens
            .get(index + 1)
            .filter(|name| token_is_identifier(&name.text))
        else {
            continue;
        };
        let end_offset = declaration_end_offset(&sanitized, token.end).unwrap_or(name.end);
        declarations.push(DeclaredItem {
            name: name.text.clone(),
            kind,
            file: source_file.path.clone(),
            span: Span {
                start_line: line_number_at(&source_file.contents, token.start),
                end_line: line_number_at(&source_file.contents, end_offset),
            },
            start_offset: token.start,
            end_offset,
            is_test_only: has_test_attribute_near(&source_file.contents, token.start),
        });
    }

    declarations
}

fn declaration_end_offset(source: &str, start: usize) -> Option<usize> {
    let bytes = source.as_bytes();
    let mut index = start;

    while index < bytes.len() {
        match bytes[index] {
            b';' => return Some(index + 1),
            b'{' => return matching_brace_offset(source, index),
            _ => index += 1,
        }
    }

    None
}

fn matching_brace_offset(source: &str, open: usize) -> Option<usize> {
    let bytes = source.as_bytes();
    let mut depth = 0_i32;

    for (index, byte) in bytes.iter().enumerate().skip(open) {
        match byte {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(index + 1);
                }
            }
            _ => {}
        }
    }

    None
}

fn collect_signature_parameter_types(signature: &str, types: &mut BTreeMap<String, String>) {
    let Some(start) = signature.find('(') else {
        return;
    };
    let Some(end) = matching_paren(signature, start) else {
        return;
    };
    for parameter in split_top_level(&signature[start + 1..end], ',') {
        let Some((name, type_name)) = parameter.split_once(':') else {
            continue;
        };
        let name = name.trim().trim_start_matches("mut ").trim();
        let type_name = normalize_type_name(type_name);
        if let Some(primitive) = primitive_numeric_type(type_name.as_str()) {
            types.insert(name.to_string(), primitive.to_string());
        }
    }
}

fn collect_let_annotation_types(body: &str, types: &mut BTreeMap<String, String>) {
    let sanitized = sanitize_source(body);
    let tokens = tokenize(&sanitized);

    for index in 0..tokens.len() {
        if tokens[index].text != "let" {
            continue;
        }
        let Some(name) = tokens
            .get(index + 1)
            .filter(|token| token_is_identifier(&token.text))
        else {
            continue;
        };
        if !tokens.get(index + 2).is_some_and(|token| token.text == ":") {
            continue;
        }
        let Some(type_token) = tokens.get(index + 3) else {
            continue;
        };
        if let Some(primitive) = primitive_numeric_type(type_token.text.as_str()) {
            types.insert(name.text.clone(), primitive.to_string());
        }
    }
}

fn normalize_type_name(type_name: &str) -> String {
    type_name
        .trim()
        .trim_start_matches('&')
        .trim_start_matches("mut")
        .trim()
        .split(|character: char| {
            character.is_whitespace()
                || matches!(character, ',' | ')' | ';' | '{' | '}' | '<' | '>')
        })
        .next()
        .unwrap_or_default()
        .trim()
        .to_string()
}

fn split_top_level(source: &str, delimiter: char) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0_i32;
    let mut start = 0_usize;

    for (index, character) in source.char_indices() {
        match character {
            '<' | '(' | '[' => depth += 1,
            '>' | ')' | ']' => depth -= 1,
            _ if character == delimiter && depth == 0 => {
                parts.push(&source[start..index]);
                start = index + character.len_utf8();
            }
            _ => {}
        }
    }
    parts.push(&source[start..]);
    parts
}

fn matching_paren(source: &str, open: usize) -> Option<usize> {
    let mut depth = 0_i32;

    for (offset, character) in source[open..].char_indices() {
        match character {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(open + offset);
                }
            }
            _ => {}
        }
    }

    None
}

fn previous_significant(tokens: &[Token], index: usize) -> Option<&Token> {
    index.checked_sub(1).and_then(|index| tokens.get(index))
}

fn skip_angle_group(tokens: &[Token], open_index: usize) -> Option<usize> {
    let close = find_matching_token(tokens, open_index, "<", ">")?;
    Some(close + 1)
}

fn is_identifier_start(character: char) -> bool {
    character == '_' || character.is_ascii_alphabetic()
}

fn is_identifier_continue(character: char) -> bool {
    character == '_' || character.is_ascii_alphanumeric()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitizer_preserves_line_numbers_and_removes_comments() {
        let source = "fun f() {\n// if true\nlet s = b\"loop\";\n}";
        let sanitized = sanitize_source(source);

        assert_eq!(source.lines().count(), sanitized.lines().count());
        assert!(!sanitized.contains("if true"));
        assert!(!sanitized.contains("loop"));
    }

    #[test]
    fn declarations_include_structs_enums_and_constants() {
        let file = SourceFile {
            path: "sources/m.move".to_string(),
            contents: r#"
module demo::m;
const FEE: u64 = 1;
public struct Vault has key { id: UID }
public enum State has drop { On, Off }
"#
            .to_string(),
        };

        let declarations = collect_file_declarations(&file);

        assert_eq!(declarations.len(), 3);
        assert!(
            declarations
                .iter()
                .any(|item| { item.name == "FEE" && item.kind == DeclaredItemKind::Const })
        );
        assert!(
            declarations
                .iter()
                .any(|item| { item.name == "Vault" && item.kind == DeclaredItemKind::Struct })
        );
        assert!(
            declarations
                .iter()
                .any(|item| { item.name == "State" && item.kind == DeclaredItemKind::Enum })
        );
    }
}
