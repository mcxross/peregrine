use std::{collections::BTreeMap, fs, path::Path};

use walkdir::WalkDir;

use crate::{
    core::{
        hash_file, logical_id, FunctionId, OperationId, SourceMapper, SourcePrecision, SourceSpan,
    },
    sui::model::{ProgramIndex, SourceFileRecord},
};

#[derive(Clone, Debug, Default)]
pub struct SuiSourceMap {
    function_spans: BTreeMap<FunctionId, SourceSpan>,
    operation_spans: BTreeMap<OperationId, SourceSpan>,
}

impl SuiSourceMap {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert_function_span(&mut self, function_id: FunctionId, span: SourceSpan) {
        self.function_spans.insert(function_id, span);
    }

    pub fn insert_operation_span(&mut self, operation_id: OperationId, span: SourceSpan) {
        self.operation_spans.insert(operation_id, span);
    }
}

impl SourceMapper for SuiSourceMap {
    fn span_for_function(&self, function_id: &FunctionId) -> Option<SourceSpan> {
        self.function_spans.get(function_id).cloned()
    }

    fn span_for_operation(&self, operation_id: &OperationId) -> Option<SourceSpan> {
        self.operation_spans.get(operation_id).cloned()
    }
}

pub fn enrich_source_spans_from_sources(program: &mut ProgramIndex, package_root: &Path) {
    let source_files = discover_source_files(package_root);
    if source_files.is_empty() {
        return;
    }

    let mut module_file_ids = BTreeMap::new();
    let mut function_spans = BTreeMap::new();
    let mut type_spans = BTreeMap::new();

    for path in source_files {
        let Ok(source) = fs::read_to_string(&path) else {
            continue;
        };
        let hash = hash_file(&path).unwrap_or_default();
        let relative = path
            .strip_prefix(package_root)
            .unwrap_or(path.as_path())
            .to_string_lossy()
            .replace('\\', "/");
        let short_hash = hash.get(..16).unwrap_or(&hash);
        let file_id = logical_id("file", [&program.package.id, &relative, short_hash]);
        if !program.files.iter().any(|file| file.id == file_id) {
            program.files.push(SourceFileRecord {
                id: file_id.clone(),
                path: path.to_string_lossy().into_owned(),
                content_hash: Some(hash),
                kind: "move_source".to_string(),
            });
        }

        let parsed = parse_source_file(&source, &file_id);
        if let Some(module_name) = parsed.module_full_name {
            module_file_ids.insert(module_name.clone(), (file_id.clone(), parsed.module_span));
            for (name, span) in parsed.function_spans {
                function_spans.insert(format!("{module_name}::{name}"), span);
            }
            for (name, span) in parsed.type_spans {
                type_spans.insert(format!("{module_name}::{name}"), span);
            }
        }
    }

    for module in &mut program.modules {
        if let Some((file_id, span)) = module_file_ids.get(&module.full_name) {
            module.file_id = Some(file_id.clone());
            module.source_span = span.clone();
        }
    }
    for function in &mut program.functions {
        if let Some(span) = function_spans.get(&function.full_name) {
            function.source_span = span.clone();
        }
    }
    for type_def in &mut program.types {
        if let Some(span) = type_spans.get(&type_def.full_name) {
            type_def.source_span = span.clone();
            for field in &mut type_def.fields {
                field.source_span = span.clone();
            }
        }
    }
}

fn discover_source_files(package_root: &Path) -> Vec<std::path::PathBuf> {
    let roots = if package_root.join("sources").is_dir() {
        vec![package_root.join("sources")]
    } else {
        vec![package_root.to_path_buf()]
    };
    let mut files = roots
        .into_iter()
        .flat_map(|root| WalkDir::new(root).into_iter().filter_map(Result::ok))
        .filter(|entry| entry.file_type().is_file())
        .map(|entry| entry.into_path())
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("move"))
        .filter(|path| {
            !path.components().any(|component| {
                matches!(
                    component.as_os_str().to_str(),
                    Some("build" | ".peregrine" | "package_summaries")
                )
            })
        })
        .collect::<Vec<_>>();
    files.sort();
    files
}

#[derive(Debug, Default)]
struct ParsedSourceFile {
    module_full_name: Option<String>,
    module_span: SourceSpan,
    function_spans: BTreeMap<String, SourceSpan>,
    type_spans: BTreeMap<String, SourceSpan>,
}

fn parse_source_file(source: &str, file_id: &str) -> ParsedSourceFile {
    let lines = source.lines().collect::<Vec<_>>();
    let total_lines = lines.len() as u32;
    let mut parsed = ParsedSourceFile::default();
    parsed.module_span = SourceSpan {
        file_id: Some(file_id.to_string()),
        start_line: Some(1),
        start_col: Some(1),
        end_line: Some(total_lines.max(1)),
        end_col: lines.last().map(|line| line.len() as u32 + 1),
        precision: SourcePrecision::File,
        ..SourceSpan::default()
    };

    for (index, line) in lines.iter().enumerate() {
        if let Some(module_name) = parse_module_name(line) {
            parsed.module_full_name = Some(module_name);
            parsed.module_span.precision = SourcePrecision::Module;
            parsed.module_span.start_line = Some(index as u32 + 1);
            break;
        }
    }

    for (index, line) in lines.iter().enumerate() {
        if let Some(function_name) = parse_decl_name(line, "fun") {
            parsed.function_spans.insert(
                function_name,
                declaration_span(&lines, index, file_id, SourcePrecision::Function),
            );
        }
        if let Some(type_name) =
            parse_decl_name(line, "struct").or_else(|| parse_decl_name(line, "enum"))
        {
            parsed.type_spans.insert(
                type_name,
                declaration_span(&lines, index, file_id, SourcePrecision::Module),
            );
        }
    }

    parsed
}

fn parse_module_name(line: &str) -> Option<String> {
    let trimmed = line.trim();
    let rest = trimmed.strip_prefix("module ")?;
    let module = rest
        .split(|character: char| character == ';' || character == '{' || character.is_whitespace())
        .next()?;
    if module.contains("::") {
        Some(module.to_string())
    } else {
        None
    }
}

fn parse_decl_name(line: &str, keyword: &str) -> Option<String> {
    let cleaned = line
        .split("//")
        .next()
        .unwrap_or(line)
        .replace('(', " (")
        .replace('<', " <");
    let tokens = cleaned
        .split_whitespace()
        .map(|token| {
            token.trim_matches(|character: char| {
                !character.is_ascii_alphanumeric() && character != '_'
            })
        })
        .filter(|token| !token.is_empty())
        .collect::<Vec<_>>();
    let keyword_index = tokens.iter().position(|token| *token == keyword)?;
    let name = tokens.get(keyword_index + 1)?;
    if name
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || character == '_')
    {
        Some((*name).to_string())
    } else {
        None
    }
}

fn declaration_span(
    lines: &[&str],
    start_index: usize,
    file_id: &str,
    precision: SourcePrecision,
) -> SourceSpan {
    let mut depth = 0i32;
    let mut saw_open = false;
    let mut end_index = start_index;
    for (index, line) in lines.iter().enumerate().skip(start_index) {
        for character in line.chars() {
            match character {
                '{' => {
                    saw_open = true;
                    depth += 1;
                }
                '}' => depth -= 1,
                _ => {}
            }
        }
        end_index = index;
        if saw_open && depth <= 0 {
            break;
        }
        if !saw_open && line.contains(';') {
            break;
        }
    }

    SourceSpan {
        file_id: Some(file_id.to_string()),
        start_line: Some(start_index as u32 + 1),
        start_col: Some(1),
        end_line: Some(end_index as u32 + 1),
        end_col: lines.get(end_index).map(|line| line.len() as u32 + 1),
        precision,
        ..SourceSpan::default()
    }
}
