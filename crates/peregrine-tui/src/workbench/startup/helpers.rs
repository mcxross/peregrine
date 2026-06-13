use crate::output::{CliStatus, CliStep};
use crate::workbench::App;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::text::Line;
use regex::Regex;
use std::ffi::OsStr;
use std::path::Path;

pub(crate) fn startup_option_line(
    app: &App,
    selected: bool,
    number: &'static str,
    label: &'static str,
) -> Line<'static> {
    let marker = if selected { ">" } else { " " };
    let style = if selected {
        app.selection_style()
    } else {
        app.base_style()
    };

    Line::styled(format!("{marker} {number}. {label}"), style)
}

pub(crate) fn is_quit_key(key: KeyEvent) -> bool {
    key.modifiers == KeyModifiers::CONTROL
        && matches!(key.code, KeyCode::Char('c') | KeyCode::Char('q'))
}

pub(crate) fn default_package_name(root: &Path) -> String {
    let raw = root
        .file_name()
        .and_then(OsStr::to_str)
        .unwrap_or("package");
    let mut name = raw
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    name = name.trim_matches('_').to_string();
    if name.is_empty() {
        name = "package".to_string();
    }
    if name.chars().next().is_some_and(|ch| ch.is_ascii_digit()) {
        name = format!("package_{name}");
    }
    name
}

pub(crate) fn package_name_error(package_name: &str) -> Option<String> {
    if package_name.is_empty() {
        return Some("Package name is required.".to_string());
    }

    if package_name
        .chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_digit())
    {
        return Some("Package name must not start with a number.".to_string());
    }

    if !package_name
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
    {
        return Some("Package name must use only letters, numbers, and underscores.".to_string());
    }

    None
}

pub(crate) fn render_cli_step_summary(step: &CliStep) -> String {
    if let Some(diagnostic) = step.diagnostics.first() {
        return diagnostic.message.clone();
    }

    if let Some(line) = step.stderr.lines().find(|line| !line.trim().is_empty()) {
        return strip_ansi_sequences(line.trim());
    }

    if let Some(line) = step.stdout.lines().find(|line| !line.trim().is_empty()) {
        return strip_ansi_sequences(line.trim());
    }

    format!("{} {}", step.name, cli_status_label(step.status))
}

fn cli_status_label(status: CliStatus) -> &'static str {
    match status {
        CliStatus::Passed => "passed",
        CliStatus::Failed => "failed",
        CliStatus::Skipped => "skipped",
    }
}

fn strip_ansi_sequences(input: &str) -> String {
    static ANSI_RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    let re = ANSI_RE.get_or_init(|| Regex::new(r"\x1b\[[0-9;]*m").expect("valid ansi regex"));
    re.replace_all(input, "").into_owned()
}
