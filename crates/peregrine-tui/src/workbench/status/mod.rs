use super::{App, render_cli_step_summary};
use crate::output::{CliStatus, CliStep};
use crate::sui::package_loader::{PackageLoadReport, PackageScannerReport, ScannerResult};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use std::time::{Duration, Instant};

pub(crate) fn package_load_status(report: &PackageLoadReport) -> String {
    if report.build.status == CliStatus::Skipped && report.test.status == CliStatus::Skipped {
        return "Package load skipped".to_string();
    }
    if report.build.status == CliStatus::Failed {
        return "Package load complete: build failed".to_string();
    }
    if report.test.status == CliStatus::Failed {
        return "Package load complete: tests failed".to_string();
    }
    "Package load complete".to_string()
}

#[cfg(test)]
pub(crate) fn package_load_status_lines(
    report: &PackageLoadReport,
    app: &App,
) -> Vec<Line<'static>> {
    let build_status = TaskStatus::from_cli(report.build.status);
    let test_status = child_task_status(build_status, TaskStatus::from_cli(report.test.status));
    let unit_total = best_scanner_count(
        &report.scanners.compiler_unit_tests,
        &report.scanners.heuristic_unit_tests,
    );
    let random_fuzz_total = best_scanner_count(
        &report.scanners.compiler_fuzz_tests,
        &report.scanners.heuristic_fuzz_tests,
    );
    let invariant_fuzz_total = best_scanner_count(
        &report.scanners.compiler_movy_invariant_tests,
        &report.scanners.heuristic_movy_invariant_tests,
    );
    let fuzz_total = random_fuzz_total + invariant_fuzz_total;
    let verification_total = best_scanner_count(
        &report.scanners.compiler_formal_verification,
        &report.scanners.heuristic_formal_verification,
    );
    let fuzz_status = child_task_status(test_status, scanner_task_status(fuzz_total));
    let verification_status =
        child_task_status(test_status, scanner_task_status(verification_total));

    let mut lines = vec![
        task_status_line(app, "build", None, build_status),
        task_status_line(app, "test", Some((unit_total, unit_total)), test_status),
        task_status_line(app, "fuzz", Some((fuzz_total, fuzz_total)), fuzz_status),
        task_status_line(
            app,
            "verification",
            Some((verification_total, verification_total)),
            verification_status,
        ),
    ];

    if report.build.status == CliStatus::Failed {
        lines.push(Line::styled(
            format!("  build: {}", render_cli_step_summary(&report.build)),
            app.style_fg(app.palette().warning),
        ));
    }
    if report.test.status == CliStatus::Failed {
        lines.push(Line::styled(
            format!("  test: {}", render_cli_step_summary(&report.test)),
            app.style_fg(app.palette().warning),
        ));
    }

    lines
}

pub(crate) fn package_load_status_spans(
    report: &PackageLoadReport,
    app: &App,
) -> Vec<Span<'static>> {
    let build_status = TaskStatus::from_cli(report.build.status);
    let test_status = child_task_status(build_status, TaskStatus::from_cli(report.test.status));
    let unit_total = best_scanner_count(
        &report.scanners.compiler_unit_tests,
        &report.scanners.heuristic_unit_tests,
    );
    let random_fuzz_total = best_scanner_count(
        &report.scanners.compiler_fuzz_tests,
        &report.scanners.heuristic_fuzz_tests,
    );
    let invariant_fuzz_total = best_scanner_count(
        &report.scanners.compiler_movy_invariant_tests,
        &report.scanners.heuristic_movy_invariant_tests,
    );
    let fuzz_total = random_fuzz_total + invariant_fuzz_total;
    let verification_total = best_scanner_count(
        &report.scanners.compiler_formal_verification,
        &report.scanners.heuristic_formal_verification,
    );
    let fuzz_status = child_task_status(test_status, scanner_task_status(fuzz_total));
    let verification_status =
        child_task_status(test_status, scanner_task_status(verification_total));

    let mut spans = vec![Span::styled("package: ", app.muted_style())];
    append_task_status_spans(&mut spans, app, "build", None, build_status);
    append_task_separator(&mut spans, app);
    append_task_status_spans(
        &mut spans,
        app,
        "test",
        Some((unit_total, unit_total)),
        test_status,
    );
    append_task_separator(&mut spans, app);
    append_task_status_spans(
        &mut spans,
        app,
        "fuzz",
        Some((fuzz_total, fuzz_total)),
        fuzz_status,
    );
    append_task_separator(&mut spans, app);
    append_task_status_spans(
        &mut spans,
        app,
        "verification",
        Some((verification_total, verification_total)),
        verification_status,
    );

    if report.build.status == CliStatus::Failed {
        append_task_separator(&mut spans, app);
        spans.push(Span::styled(
            format!("build: {}", render_cli_step_summary(&report.build)),
            app.style_fg(app.palette().warning),
        ));
    }
    if report.test.status == CliStatus::Failed {
        append_task_separator(&mut spans, app);
        spans.push(Span::styled(
            format!("test: {}", render_cli_step_summary(&report.test)),
            app.style_fg(app.palette().warning),
        ));
    }

    spans
}

pub(crate) fn package_load_spinner(started_at: Instant) -> &'static str {
    const FRAMES: [&str; 4] = ["-", "\\", "|", "/"];
    let frame = (started_at.elapsed().as_millis() / 125) as usize % FRAMES.len();
    FRAMES[frame]
}

pub(crate) fn format_elapsed(duration: Duration) -> String {
    let seconds = duration.as_secs();
    let minutes = seconds / 60;
    let seconds = seconds % 60;
    if minutes > 0 {
        format!("{minutes}m {seconds:02}s")
    } else {
        format!("{seconds}s")
    }
}

#[derive(Clone, Copy)]
enum TaskStatus {
    Passed,
    Failed,
    Skipped,
}

impl TaskStatus {
    fn from_cli(status: CliStatus) -> Self {
        match status {
            CliStatus::Passed => Self::Passed,
            CliStatus::Failed => Self::Failed,
            CliStatus::Skipped => Self::Skipped,
        }
    }
}

fn child_task_status(parent: TaskStatus, own: TaskStatus) -> TaskStatus {
    match parent {
        TaskStatus::Passed => own,
        TaskStatus::Failed | TaskStatus::Skipped => TaskStatus::Failed,
    }
}

fn scanner_task_status(total: usize) -> TaskStatus {
    if total > 0 {
        TaskStatus::Passed
    } else {
        TaskStatus::Failed
    }
}

#[cfg(test)]
fn task_status_line(
    app: &App,
    label: &'static str,
    counts: Option<(usize, usize)>,
    status: TaskStatus,
) -> Line<'static> {
    let (marker, style) = match status {
        TaskStatus::Passed => ("✓", app.style_fg(app.palette().success)),
        TaskStatus::Failed => ("✕", app.style_fg(app.palette().warning)),
        TaskStatus::Skipped => ("-", app.muted_style()),
    };
    let count_suffix = counts
        .map(|(complete, total)| format!(" ({complete}/{total})"))
        .unwrap_or_default();

    Line::from(vec![
        Span::styled(marker.to_string(), style),
        Span::raw(" "),
        Span::styled(format!("{label}{count_suffix}"), app.base_style()),
    ])
}

fn append_task_status_spans(
    spans: &mut Vec<Span<'static>>,
    app: &App,
    label: &'static str,
    counts: Option<(usize, usize)>,
    status: TaskStatus,
) {
    let (marker, style) = match status {
        TaskStatus::Passed => ("✓", app.style_fg(app.palette().success)),
        TaskStatus::Failed => ("✕", app.style_fg(app.palette().warning)),
        TaskStatus::Skipped => ("-", app.muted_style()),
    };
    let count_suffix = counts
        .map(|(complete, total)| format!(" ({complete}/{total})"))
        .unwrap_or_default();

    spans.push(Span::styled(marker.to_string(), style));
    spans.push(Span::raw(" "));
    spans.push(Span::styled(
        format!("{label}{count_suffix}"),
        app.base_style(),
    ));
}

fn append_task_separator(spans: &mut Vec<Span<'static>>, app: &App) {
    spans.push(Span::styled(" | ", app.muted_style()));
}

fn best_scanner_count(compiler: &ScannerResult, heuristic: &ScannerResult) -> usize {
    scanner_count(compiler).unwrap_or_else(|| scanner_count(heuristic).unwrap_or_default())
}

fn scanner_count(result: &ScannerResult) -> Option<usize> {
    match result {
        ScannerResult::Found { count } => Some(*count),
        ScannerResult::NotFound => Some(0),
        ScannerResult::Failed { .. } | ScannerResult::Unavailable { .. } => None,
    }
}

fn cli_status_label(status: CliStatus) -> &'static str {
    match status {
        CliStatus::Passed => "passed",
        CliStatus::Failed => "failed",
        CliStatus::Skipped => "skipped",
    }
}

pub(crate) fn focused_title(title: &str, focused: bool) -> String {
    if focused {
        format!("* {title}")
    } else {
        title.to_string()
    }
}
