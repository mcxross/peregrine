use super::super::call_graph::MoveSourceSpan;

pub(super) fn source_for_range(source: &str, start: usize, end: usize) -> Option<String> {
    if start <= end && end <= source.len() {
        Some(source[start..end].to_string())
    } else {
        None
    }
}

pub(super) fn source_span(
    source: &str,
    file_path: &str,
    start: usize,
    end: usize,
) -> MoveSourceSpan {
    MoveSourceSpan {
        file_path: file_path.to_string(),
        start_line: line_number_at(source, start),
        end_line: line_number_at(source, end),
        start_byte: start,
        end_byte: end,
    }
}

pub(super) fn summary_span() -> MoveSourceSpan {
    MoveSourceSpan {
        file_path: "package_summaries".to_string(),
        start_line: 0,
        end_line: 0,
        start_byte: 0,
        end_byte: 0,
    }
}

fn line_number_at(source: &str, offset: usize) -> usize {
    source
        .get(..offset.min(source.len()))
        .unwrap_or_default()
        .bytes()
        .filter(|byte| *byte == b'\n')
        .count()
        + 1
}
