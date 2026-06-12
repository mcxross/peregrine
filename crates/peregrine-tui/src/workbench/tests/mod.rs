use super::{
    load_editor_mode_from_home, load_theme_from_home, App, AppMode, CommandInput, EditorBuffer,
    EditorMode, Explorer, FocusPane, TuiSettings, VimState, WorkbenchExit, WorkbenchTab,
};
use crate::bootstrap::is_helper_arg;
use crate::chat;
use crate::navigation::{Navigation, NavigationCommand};
use crate::output::{CliStatus, CliStep};
use crate::helper_args;
use crate::sui::package_loader::{
    failed_startup_step, PackageCreateReport, PackageInspection, PackageLoadReport,
    PackageScannerReport, ScannerResult, WorkbenchTrustResolution,
};
use crate::workbench::status::package_load_status_lines;
use crate::workbench::{filter_type_graph, render_type_graph_text};
use crate::sui::project::{BytecodeTarget, CliContext};
use crate::theme::{Theme, ThemeName, ThemePalette};
use crate::workbench::prelude::*;
use crate::workbench_render::{is_markdown_path, render_workbench_document};
use peregrine_config::CONFIG_TOML_FILE;
use peregrine_mcp_protocol::{
    BytecodeViewResponse, GraphsResponse, MoveBytecodeModuleView, MoveTypeGraph,
    MoveTypeGraphEdge, MoveTypeGraphNode, MoveUnresolvedType, PackageArgs as McpPackageArgs,
    tool_name,
};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::crossterm::event::{
    Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent,
    MouseEventKind,
};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Frame;
use serde_json::Value;
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

static CREATED_PACKAGE_TRUST_PERSISTED: AtomicBool = AtomicBool::new(false);

#[test]
fn config_missing_defaults_to_standard_mode() {
    let temp = tempfile::tempdir().expect("temp dir");
    assert_eq!(
        load_editor_mode_from_home(temp.path()),
        EditorMode::Standard
    );
}

#[test]
fn config_false_defaults_to_standard_mode() {
    let temp = tempfile::tempdir().expect("temp dir");
    fs::write(
        temp.path().join(CONFIG_TOML_FILE),
        "[tui]\nvim_mode_default = false\n",
    )
    .expect("write config");
    assert_eq!(
        load_editor_mode_from_home(temp.path()),
        EditorMode::Standard
    );
}

#[test]
fn config_true_defaults_to_vim_mode() {
    let temp = tempfile::tempdir().expect("temp dir");
    fs::write(
        temp.path().join(CONFIG_TOML_FILE),
        "[tui]\nvim_mode_default = true\n",
    )
    .expect("write config");
    assert_eq!(load_editor_mode_from_home(temp.path()), EditorMode::Vim);
}

#[test]
fn config_theme_loads_named_theme() {
    let temp = tempfile::tempdir().expect("temp dir");
    fs::write(
        temp.path().join(CONFIG_TOML_FILE),
        "[tui]\ntheme = \"zero-day\"\n",
    )
    .expect("write config");

    assert_eq!(load_theme_from_home(temp.path()).name, ThemeName::ZeroDay);
}

#[test]
fn config_invalid_theme_defaults_to_peregrine_night() {
    let temp = tempfile::tempdir().expect("temp dir");
    fs::write(
        temp.path().join(CONFIG_TOML_FILE),
        "[tui]\ntheme = \"not-a-theme\"\n",
    )
    .expect("write config");

    assert_eq!(
        load_theme_from_home(temp.path()).name,
        ThemeName::PeregrineNight
    );
}

#[test]
fn helper_args_bypass_mode_dispatch() {
    assert!(is_helper_arg(&OsString::from(
        helper_args::BYTECODE_VIEWER_HELPER_ARG
    )));
}

#[test]
fn invalid_package_screen_renders_create_and_proceed_options() {
    let temp = tempfile::tempdir().expect("temp dir");
    let mut app = app_with_startup(temp.path(), None);
    let backend = TestBackend::new(120, 30);
    let mut terminal = Terminal::new(backend).expect("terminal");

    terminal.draw(|frame| app.render(frame)).expect("draw");
    let rendered = buffer_to_string(terminal.backend().buffer());

    assert!(rendered.contains("does not appear to contain a valid Move package"));
    assert!(rendered.contains("Create a new Move package"));
    assert!(rendered.contains("Proceed anyway using the selected directory"));
}

#[test]
fn create_flow_asks_for_package_name() {
    let temp = tempfile::tempdir().expect("temp dir");
    let mut app = app_with_startup(temp.path(), None);

    app.handle_key_event(key(KeyCode::Enter));

    let WorkbenchStartupState::PackageNameEntry(prompt) = &app.startup else {
        panic!("expected package name prompt");
    };
    assert_eq!(prompt.parent, temp.path().canonicalize().unwrap());
    assert_eq!(prompt.input.text, default_package_name(temp.path()));
}

#[test]
fn created_package_switches_root_and_skips_immediate_trust_prompt() {
    CREATED_PACKAGE_TRUST_PERSISTED.store(false, AtomicOrdering::SeqCst);
    let temp = tempfile::tempdir().expect("temp dir");
    let parent = temp.path().canonicalize().unwrap();
    let package_root = parent.join("demo");
    fs::create_dir_all(package_root.join("sources")).expect("sources");
    fs::write(
        package_root.join("Move.toml"),
        "[package]\nname = \"demo\"\n",
    )
    .expect("manifest");
    let mut app = app_with_startup(parent.as_path(), None);
    app.created_package_trust_persister = record_created_package_trust;
    app.package_loader = fake_package_loader;
    let trust_resolution = test_trust_resolution(parent.as_path(), None);

    app.apply_create_package_report(
        parent,
        "demo".to_string(),
        trust_resolution,
        "missing manifest".to_string(),
        PackageCreateReport {
            step: passed_step("new-package"),
            package_root: package_root.clone(),
        },
    );
    drain_startup_until_complete(&mut app);

    assert_eq!(app.explorer.root, package_root.canonicalize().unwrap());
    assert!(CREATED_PACKAGE_TRUST_PERSISTED.load(AtomicOrdering::SeqCst));
    assert!(!matches!(
        app.startup,
        WorkbenchStartupState::TrustDecision(_)
    ));
    assert!(matches!(app.startup, WorkbenchStartupState::Workbench));
    assert!(app.package_load_report.is_some());
}

#[test]
fn valid_package_waits_for_trust_before_loading() {
    let temp = tempfile::tempdir().expect("temp dir");
    fs::write(
        temp.path().join("Move.toml"),
        "[package]\nname = \"demo\"\n",
    )
    .expect("manifest");
    let app = app_with_startup(temp.path(), None);

    assert!(matches!(
        app.startup,
        WorkbenchStartupState::TrustDecision(_)
    ));
    assert!(app.startup_task_rx.is_none());
    assert!(app.package_load_report.is_none());
}

#[test]
fn trust_denied_skips_build_test_and_scanners() {
    let temp = tempfile::tempdir().expect("temp dir");
    fs::write(
        temp.path().join("Move.toml"),
        "[package]\nname = \"demo\"\n",
    )
    .expect("manifest");
    let mut app = app_with_startup(temp.path(), None);

    app.handle_key_event(key(KeyCode::Char('2')));

    assert!(matches!(app.startup, WorkbenchStartupState::Workbench));
    let report = app
        .package_load_report
        .as_ref()
        .expect("expected skipped package load report");
    assert_eq!(report.build.status, CliStatus::Skipped);
    assert_eq!(report.test.status, CliStatus::Skipped);
    assert!(matches!(
        report.scanners.compiler_unit_tests,
        ScannerResult::Unavailable { .. }
    ));
    assert!(app.startup_task_rx.is_none());
}

#[test]
fn package_status_uses_markers_counts_and_parent_failure_relationships() {
    let temp = tempfile::tempdir().expect("temp dir");
    let app = App::new(temp.path(), EditorMode::Standard).expect("app");
    let report = PackageLoadReport {
        package_root: temp.path().to_path_buf(),
        build: passed_step("build"),
        test: failed_startup_step("test", "unit test error"),
        scanners: PackageScannerReport {
            compiler_unit_tests: ScannerResult::Found { count: 40 },
            compiler_movy_invariant_tests: ScannerResult::NotFound,
            compiler_fuzz_tests: ScannerResult::Found { count: 4 },
            compiler_formal_verification: ScannerResult::Found { count: 2 },
            heuristic_unit_tests: ScannerResult::Found { count: 40 },
            heuristic_movy_invariant_tests: ScannerResult::NotFound,
            heuristic_fuzz_tests: ScannerResult::Found { count: 4 },
            heuristic_formal_verification: ScannerResult::Found { count: 2 },
        },
    };
    let lines = package_load_status_lines(&report, &app);
    let rendered = lines_to_plain_text(&lines);

    assert!(rendered.contains("✓ build"));
    assert!(rendered.contains("✕ test (40/40)"));
    assert!(rendered.contains("✕ fuzz (4/4)"));
    assert!(rendered.contains("✕ verification (2/2)"));
    assert!(!rendered.contains("passed"));
    assert!(!rendered.contains("build: passed"));
    assert!(!rendered.contains("test: failed"));
}

#[test]
fn build_failure_marks_all_child_tasks_failed() {
    let temp = tempfile::tempdir().expect("temp dir");
    let app = App::new(temp.path(), EditorMode::Standard).expect("app");
    let report = PackageLoadReport {
        package_root: temp.path().to_path_buf(),
        build: failed_startup_step("build", "compiler failed"),
        test: passed_step("test"),
        scanners: PackageScannerReport {
            compiler_unit_tests: ScannerResult::Unavailable {
                reason: "build/compiler output is unavailable".to_string(),
            },
            compiler_movy_invariant_tests: ScannerResult::Unavailable {
                reason: "build/compiler output is unavailable".to_string(),
            },
            compiler_fuzz_tests: ScannerResult::Unavailable {
                reason: "build/compiler output is unavailable".to_string(),
            },
            compiler_formal_verification: ScannerResult::Unavailable {
                reason: "build/compiler output is unavailable".to_string(),
            },
            heuristic_unit_tests: ScannerResult::Found { count: 40 },
            heuristic_movy_invariant_tests: ScannerResult::NotFound,
            heuristic_fuzz_tests: ScannerResult::Found { count: 4 },
            heuristic_formal_verification: ScannerResult::Found { count: 2 },
        },
    };
    let lines = package_load_status_lines(&report, &app);
    let rendered = lines_to_plain_text(&lines);

    assert!(rendered.contains("✕ build"));
    assert!(rendered.contains("✕ test (40/40)"));
    assert!(rendered.contains("✕ fuzz (4/4)"));
    assert!(rendered.contains("✕ verification (2/2)"));
}

#[test]
fn package_load_running_renders_spinner_in_bottom_bar() {
    let temp = tempfile::tempdir().expect("temp dir");
    let mut app = App::new(temp.path(), EditorMode::Standard).expect("app");
    app.startup = WorkbenchStartupState::PackageLoadRunning(PackageLoadRunningState {
        message: "Building, testing, and scanning demo".to_string(),
        started_at: Instant::now(),
    });

    let backend = TestBackend::new(120, 30);
    let mut terminal = Terminal::new(backend).expect("terminal");
    terminal.draw(|frame| app.render(frame)).expect("draw");
    let rendered = buffer_to_string(terminal.backend().buffer());

    assert!(rendered.contains("Explorer"));
    assert!(rendered.contains("running"));
    assert!(!rendered.contains("task:"));
    assert!(!rendered.contains("Building"));
    assert!(!rendered.contains("scanning demo"));
    assert!(!rendered.contains("Package load"));
    assert!(!rendered.contains("Working in the background"));
}

#[test]
fn package_load_completion_renders_in_bottom_bar_without_modal() {
    let temp = tempfile::tempdir().expect("temp dir");
    let mut app = App::new(temp.path(), EditorMode::Standard).expect("app");
    let report = PackageLoadReport {
        package_root: temp.path().to_path_buf(),
        build: passed_step("build"),
        test: passed_step("test"),
        scanners: PackageScannerReport {
            compiler_unit_tests: ScannerResult::Found { count: 1 },
            compiler_movy_invariant_tests: ScannerResult::NotFound,
            compiler_fuzz_tests: ScannerResult::NotFound,
            compiler_formal_verification: ScannerResult::NotFound,
            heuristic_unit_tests: ScannerResult::Found { count: 1 },
            heuristic_movy_invariant_tests: ScannerResult::NotFound,
            heuristic_fuzz_tests: ScannerResult::NotFound,
            heuristic_formal_verification: ScannerResult::NotFound,
        },
    };
    app.finish_package_load(report);

    let backend = TestBackend::new(120, 30);
    let mut terminal = Terminal::new(backend).expect("terminal");
    terminal.draw(|frame| app.render(frame)).expect("draw");
    let rendered = buffer_to_string(terminal.backend().buffer());

    assert!(matches!(app.startup, WorkbenchStartupState::Workbench));
    assert!(rendered.contains("Explorer"));
    assert!(rendered.contains("✓ build"));
    assert!(rendered.contains("✓ test (1/1)"));
    assert!(rendered.contains("✕ fuzz (0/0)"));
    assert!(rendered.contains("✕ verification (0/0)"));
    assert!(!rendered.contains(temp.path().to_string_lossy().as_ref()));
    assert!(!rendered.contains("Package Load Complete"));
    assert!(!rendered.contains("Press Enter to open the workbench"));
}

#[test]
fn explorer_sorts_directories_before_files() {
    let temp = tempfile::tempdir().expect("temp dir");
    fs::create_dir(temp.path().join("z_dir")).expect("create dir");
    fs::create_dir(temp.path().join("a_dir")).expect("create dir");
    fs::write(temp.path().join("a_file.move"), "").expect("write file");
    fs::write(temp.path().join("z_file.move"), "").expect("write file");

    let explorer = Explorer::new(temp.path()).expect("explorer");
    let names = explorer
        .visible_entries()
        .iter()
        .skip(1)
        .map(|entry| entry.name.as_str())
        .collect::<Vec<_>>();
    assert_eq!(names, ["a_dir", "z_dir", "a_file.move", "z_file.move"]);
}

#[test]
fn explorer_toggles_directory_expansion() {
    let temp = tempfile::tempdir().expect("temp dir");
    fs::create_dir(temp.path().join("src")).expect("create dir");
    fs::write(temp.path().join("src").join("main.move"), "").expect("write file");

    let mut explorer = Explorer::new(temp.path()).expect("explorer");
    explorer.select_next();
    assert_eq!(
        explorer.activate_selected(),
        ExplorerAction::ToggledDirectory
    );
    assert!(
        explorer
            .visible_entries()
            .iter()
            .any(|entry| entry.name == "main.move")
    );
    assert_eq!(
        explorer.activate_selected(),
        ExplorerAction::ToggledDirectory
    );
    assert!(
        !explorer
            .visible_entries()
            .iter()
            .any(|entry| entry.name == "main.move")
    );
}

#[test]
fn app_blocks_opening_another_file_when_dirty() {
    let temp = tempfile::tempdir().expect("temp dir");
    let first = temp.path().join("first.move");
    let second = temp.path().join("second.move");
    fs::write(&first, "first").expect("write file");
    fs::write(&second, "second").expect("write file");
    let mut app = App::new(temp.path(), EditorMode::Standard).expect("app");

    app.open_file(first.clone());
    app.editor.insert_char('!');
    app.open_file(second.clone());

    assert_eq!(app.editor.path.as_ref(), Some(&first));
    assert!(app.status.contains("Unsaved changes"));
}

#[test]
fn bytecode_options_use_open_move_file_package_and_module() {
    let temp = tempfile::tempdir().expect("temp dir");
    fs::create_dir_all(temp.path().join("pkg/sources")).expect("sources");
    fs::write(
        temp.path().join("pkg/Move.toml"),
        "[package]\nname = \"demo\"\n",
    )
    .expect("manifest");
    let source = temp.path().join("pkg/sources/not_filename.move");
    fs::write(&source, "module demo::actual { public fun ping() {} }").expect("source");
    let mut app = App::new(temp.path(), EditorMode::Standard).expect("app");
    app.open_file(source.clone());

    let options = app.current_bytecode_options().expect("bytecode options");
    let target = options.targets.first().expect("target");

    assert_eq!(
        options.key.package_root,
        temp.path().join("pkg").canonicalize().unwrap()
    );
    assert_eq!(options.targets.len(), 1);
    assert_eq!(target.module_name, "actual");
    assert_eq!(target.source_path, source.canonicalize().unwrap());
}

#[test]
fn bytecode_options_block_dirty_editor() {
    let temp = tempfile::tempdir().expect("temp dir");
    fs::create_dir_all(temp.path().join("sources")).expect("sources");
    fs::write(
        temp.path().join("Move.toml"),
        "[package]\nname = \"demo\"\n",
    )
    .expect("manifest");
    let source = temp.path().join("sources/m.move");
    fs::write(&source, "module demo::m { public fun ping() {} }").expect("source");
    let mut app = App::new(temp.path(), EditorMode::Standard).expect("app");
    app.open_file(source);
    app.editor.insert_char(' ');

    let error = app.current_bytecode_options().expect_err("dirty editor");

    assert!(error.contains("Save the current file"));
}

#[test]
fn bytecode_package_root_lists_modules_for_selection() {
    let temp = tempfile::tempdir().expect("temp dir");
    fs::create_dir_all(temp.path().join("sources")).expect("sources");
    fs::write(
        temp.path().join("Move.toml"),
        "[package]\nname = \"demo\"\n",
    )
    .expect("manifest");
    fs::write(
        temp.path().join("sources/a.move"),
        "module demo::a { public fun ping() {} }",
    )
    .expect("source a");
    fs::write(
        temp.path().join("sources/b.move"),
        "module demo::b { public fun pong() {} }",
    )
    .expect("source b");
    let mut app = App::new(temp.path(), EditorMode::Standard).expect("app");

    app.ensure_bytecode_session();

    let BytecodePane::Selecting(selector) = &mut app.bytecode else {
        panic!("expected module selector");
    };

    assert_eq!(
        selector
            .targets
            .iter()
            .map(|target| target.module_name.as_str())
            .collect::<Vec<_>>(),
        vec!["a", "b"]
    );

    selector.handle_key(key(KeyCode::Down));
    let request = selector.selected_request().expect("selected request");

    assert_eq!(request.key.module_name, "b");
}

#[test]
fn selecting_bytecode_tab_is_lazy_until_enter() {
    let temp = tempfile::tempdir().expect("temp dir");
    fs::create_dir_all(temp.path().join("sources")).expect("sources");
    fs::write(
        temp.path().join("Move.toml"),
        "[package]\nname = \"demo\"\n",
    )
    .expect("manifest");
    fs::write(
        temp.path().join("sources/a.move"),
        "module demo::a { public fun ping() {} }",
    )
    .expect("source");
    fs::write(
        temp.path().join("sources/b.move"),
        "module demo::b { public fun pong() {} }",
    )
    .expect("source");
    let mut app = App::new(temp.path(), EditorMode::Standard).expect("app");

    app.set_active_tab(WorkbenchTab::Bytecode);
    app.set_focus(FocusPane::Editor);

    assert_eq!(app.active_tab, WorkbenchTab::Bytecode);
    assert!(matches!(app.bytecode, BytecodePane::Empty));
    assert!(app.bytecode_loader_rx.is_none());

    app.handle_key_event(key(KeyCode::Enter));
    assert!(matches!(app.bytecode, BytecodePane::Selecting(_)));
}

#[test]
fn stale_bytecode_load_result_is_ignored_after_invalidation() {
    let temp = tempfile::tempdir().expect("temp dir");
    let mut app = App::new(temp.path(), EditorMode::Standard).expect("app");
    let key = BytecodeTargetKey {
        package_root: temp.path().to_path_buf(),
        module_name: "m".to_string(),
        source_path: temp.path().join("sources/m.move"),
    };
    app.bytecode = BytecodePane::Loading(BytecodeLoadState {
        key: key.clone(),
        package_name: "demo".to_string(),
        module_name: "m".to_string(),
        stamp: BytecodeCacheStamp::default(),
        epoch: 1,
    });

    app.invalidate_bytecode();
    app.apply_bytecode_load_result(BytecodeLoadResult {
        epoch: 1,
        key,
        stamp: BytecodeCacheStamp::default(),
        result: Err("late result".to_string()),
    });

    assert!(matches!(app.bytecode, BytecodePane::Empty));
    assert!(app.bytecode_cache.is_empty());
}

#[test]
fn editor_standard_edits_and_saves() {
    let temp = tempfile::tempdir().expect("temp dir");
    let file = temp.path().join("module.move");
    fs::write(&file, "abc").expect("write file");
    let mut editor = EditorBuffer::new_empty();
    editor.open_file(&file).expect("open file");

    editor.handle_standard_key(key(KeyCode::End));
    editor.handle_standard_key(key(KeyCode::Char('d')));
    editor.handle_standard_key(key(KeyCode::Enter));
    editor.handle_standard_key(key(KeyCode::Char('e')));
    editor.save().expect("save");

    let saved = fs::read_to_string(file).expect("read file");
    assert_eq!(saved, "abcd\ne");
    assert!(!editor.dirty);
}

#[test]
fn editor_undo_restores_previous_snapshot() {
    let mut editor = EditorBuffer::new_empty();
    editor.insert_char('a');
    editor.insert_char('b');
    editor.undo();
    assert_eq!(editor.text(), "a");
}

#[test]
fn vim_subset_supports_insert_delete_yank_paste_and_undo() {
    let temp = tempfile::tempdir().expect("temp dir");
    let file = temp.path().join("module.move");
    fs::write(&file, "one\ntwo").expect("write file");
    let mut app = App::new(temp.path(), EditorMode::Vim).expect("app");
    app.open_file(file);

    app.handle_key_event(key(KeyCode::Char('j')));
    app.handle_key_event(key(KeyCode::Char('y')));
    app.handle_key_event(key(KeyCode::Char('y')));
    app.handle_key_event(key(KeyCode::Char('p')));
    assert_eq!(app.editor.text(), "one\ntwo\ntwo");

    app.handle_key_event(key(KeyCode::Char('d')));
    app.handle_key_event(key(KeyCode::Char('d')));
    assert_eq!(app.editor.text(), "one\ntwo");

    app.handle_key_event(key(KeyCode::Char('u')));
    assert_eq!(app.editor.text(), "one\ntwo\ntwo");

    app.handle_key_event(key(KeyCode::Char('i')));
    assert_eq!(app.vim_state, VimState::Insert);
    app.handle_key_event(key(KeyCode::Char('!')));
    assert!(app.editor.text().contains('!'));
    app.handle_key_event(key(KeyCode::Esc));
    assert_eq!(app.vim_state, VimState::Normal);
}

#[test]
fn markdown_preview_switches_to_raw_source_in_standard_editing() {
    let temp = tempfile::tempdir().unwrap_or_else(|error| panic!("temp dir: {error}"));
    let file = temp.path().join("README.md");
    fs::write(&file, "# Title\n\nbody").unwrap_or_else(|error| panic!("write file: {error}"));
    let mut app = App::new(temp.path(), EditorMode::Standard)
        .unwrap_or_else(|error| panic!("app: {error}"));
    app.open_file(file);
    app.focus = FocusPane::Editor;

    assert!(app.markdown_preview_enabled());
    let source = app.editor.text();
    let markdown_preview = app.markdown_preview_enabled();
    let rendered = app.rendered_editor_document(&source, 80, markdown_preview);
    assert_eq!(
        rendered.mode,
        crate::workbench_render::WorkbenchRenderMode::MarkdownPreview
    );
    assert!(!rendered.show_gutter);
    assert!(!rendered.show_cursor);

    app.handle_key_event(key(KeyCode::Char('i')));
    assert!(app.standard_editor_editing);
    assert!(!app.markdown_preview_enabled());
    let source = app.editor.text();
    let markdown_preview = app.markdown_preview_enabled();
    let rendered = app.rendered_editor_document(&source, 80, markdown_preview);
    assert_eq!(
        rendered.mode,
        crate::workbench_render::WorkbenchRenderMode::CommonSyntax
    );
    assert!(rendered.show_gutter);
    assert!(rendered.show_cursor);
}

#[test]
fn markdown_preview_switches_to_raw_source_in_vim_insert() {
    let temp = tempfile::tempdir().unwrap_or_else(|error| panic!("temp dir: {error}"));
    let file = temp.path().join("README.md");
    fs::write(&file, "# Title").unwrap_or_else(|error| panic!("write file: {error}"));
    let mut app =
        App::new(temp.path(), EditorMode::Vim).unwrap_or_else(|error| panic!("app: {error}"));
    app.open_file(file);
    app.focus = FocusPane::Editor;

    assert!(app.markdown_preview_enabled());
    let source = app.editor.text();
    let markdown_preview = app.markdown_preview_enabled();
    let rendered = app.rendered_editor_document(&source, 80, markdown_preview);
    assert_eq!(
        rendered.mode,
        crate::workbench_render::WorkbenchRenderMode::MarkdownPreview
    );

    app.handle_key_event(key(KeyCode::Char('i')));
    assert_eq!(app.vim_state, VimState::Insert);
    assert!(!app.markdown_preview_enabled());
    let source = app.editor.text();
    let markdown_preview = app.markdown_preview_enabled();
    let rendered = app.rendered_editor_document(&source, 80, markdown_preview);
    assert_eq!(
        rendered.mode,
        crate::workbench_render::WorkbenchRenderMode::CommonSyntax
    );

    app.handle_key_event(key(KeyCode::Esc));
    assert_eq!(app.vim_state, VimState::Normal);
    assert!(app.markdown_preview_enabled());
}

#[test]
fn theme_change_invalidates_editor_render_cache() {
    let temp = tempfile::tempdir().unwrap_or_else(|error| panic!("temp dir: {error}"));
    let file = temp.path().join("sources.move");
    fs::write(&file, "module demo::m { public fun ping() {} }")
        .unwrap_or_else(|error| panic!("write file: {error}"));
    let mut app = App::new(temp.path(), EditorMode::Standard)
        .unwrap_or_else(|error| panic!("app: {error}"));
    app.open_file(file);

    let source = app.editor.text();
    let markdown_preview = app.markdown_preview_enabled();
    let _ = app.rendered_editor_document(&source, 80, markdown_preview);
    assert!(app.editor_render_cache.is_some());

    app.next_theme();
    assert!(app.editor_render_cache.is_none());
}

#[test]
fn chat_theme_selection_updates_workbench_theme() {
    let temp = tempfile::tempdir().expect("temp dir");
    let mut app = App::new(temp.path(), EditorMode::Standard).expect("app");

    app.apply_chat_action(chat::ChatAction::ThemeSelected("zero-day".to_string()));

    assert_eq!(app.theme.current_name(), ThemeName::ZeroDay);
    assert_eq!(app.status, "Theme: Zero Day");
}

#[test]
fn navigation_moves_between_workbench_sections() {
    let temp = tempfile::tempdir().expect("temp dir");
    let mut app = App::new(temp.path(), EditorMode::Standard).expect("app");

    assert_eq!(app.focus, FocusPane::Explorer);
    workbench_nav(&mut app, KeyCode::Char('l'));
    assert_eq!(app.focus, FocusPane::Editor);

    workbench_nav(&mut app, KeyCode::Char('j'));
    assert_eq!(app.focus, FocusPane::Input);

    workbench_nav(&mut app, KeyCode::Char('l'));
    assert_eq!(app.focus, FocusPane::Editor);

    workbench_nav(&mut app, KeyCode::Char('h'));
    assert_eq!(app.focus, FocusPane::Explorer);

    workbench_nav(&mut app, KeyCode::Char('l'));
    assert_eq!(app.focus, FocusPane::Editor);

    workbench_nav(&mut app, KeyCode::Char('k'));
    assert_eq!(app.focus, FocusPane::Tabs);

    workbench_nav(&mut app, KeyCode::Char('h'));
    assert_eq!(app.focus, FocusPane::Explorer);
}

#[test]
fn tab_bar_focus_changes_active_workbench_tab() {
    let temp = tempfile::tempdir().expect("temp dir");
    let mut app = App::new(temp.path(), EditorMode::Standard).expect("app");

    workbench_nav(&mut app, KeyCode::Char('t'));
    assert_eq!(app.focus, FocusPane::Tabs);
    app.handle_key_event(key(KeyCode::Right));
    assert_eq!(app.active_tab, WorkbenchTab::Bytecode);
    app.handle_key_event(key(KeyCode::Left));
    assert_eq!(app.active_tab, WorkbenchTab::Code);
    app.handle_key_event(key(KeyCode::Enter));
    assert_eq!(app.focus, FocusPane::Editor);
}

#[test]
fn workbench_never_renders_inspector_panel() {
    let temp = tempfile::tempdir().expect("temp dir");
    let mut app = App::new(temp.path(), EditorMode::Standard).expect("app");

    app.set_active_tab(WorkbenchTab::Bytecode);

    let backend = TestBackend::new(120, 30);
    let mut terminal = Terminal::new(backend).expect("terminal");
    terminal.draw(|frame| app.render(frame)).expect("draw");
    let rendered = buffer_to_string(terminal.backend().buffer());

    assert!(rendered.contains("Bytecode"));
    assert!(!rendered.contains("Inspector"));

    app.set_active_tab(WorkbenchTab::Cfg);

    let backend = TestBackend::new(120, 30);
    let mut terminal = Terminal::new(backend).expect("terminal");
    terminal.draw(|frame| app.render(frame)).expect("draw");
    let rendered = buffer_to_string(terminal.backend().buffer());

    assert!(rendered.contains("cfg is not loaded"));
    assert!(rendered.contains("package: no status yet"));
    assert!(!rendered.contains("Inspector"));
}

#[test]
fn removed_inspector_shortcut_is_unbound() {
    let temp = tempfile::tempdir().expect("temp dir");
    let mut app = App::new(temp.path(), EditorMode::Standard).expect("app");
    app.focus = FocusPane::Editor;

    workbench_nav(&mut app, KeyCode::Char('p'));

    assert_eq!(app.focus, FocusPane::Editor);
    assert_eq!(app.status, crate::navigation::WORKBENCH_UNBOUND);
}

#[test]
fn editor_tab_key_still_inserts_tab_character() {
    let temp = tempfile::tempdir().expect("temp dir");
    let mut app = App::new(temp.path(), EditorMode::Standard).expect("app");

    workbench_nav(&mut app, KeyCode::Char('c'));
    app.handle_key_event(key(KeyCode::Enter));
    app.handle_key_event(key(KeyCode::Tab));

    assert_eq!(app.focus, FocusPane::Editor);
    assert_eq!(app.editor.text(), "\t");
}

#[test]
fn standard_editor_view_navigation_does_not_type() {
    let temp = tempfile::tempdir().expect("temp dir");
    let mut app = App::new(temp.path(), EditorMode::Standard).expect("app");
    app.focus = FocusPane::Editor;

    app.handle_key_event(key(KeyCode::Char('j')));
    assert_eq!(app.focus, FocusPane::Input);
    assert_eq!(app.editor.text(), "");

    app.focus = FocusPane::Editor;
    app.handle_key_event(key(KeyCode::Char('l')));
    assert_eq!(app.focus, FocusPane::Editor);
    assert_eq!(app.editor.text(), "");

    app.focus = FocusPane::Editor;
    app.handle_key_event(key_with_modifiers(KeyCode::Right, KeyModifiers::CONTROL));
    assert_eq!(app.focus, FocusPane::Editor);
    assert_eq!(app.editor.text(), "");
}

#[test]
fn standard_editor_editing_is_explicit_and_esc_returns_to_navigation() {
    let temp = tempfile::tempdir().expect("temp dir");
    let mut app = App::new(temp.path(), EditorMode::Standard).expect("app");
    app.focus = FocusPane::Editor;

    app.handle_key_event(key(KeyCode::Char('i')));
    assert!(app.standard_editor_editing);
    assert_eq!(app.editor.text(), "");

    app.handle_key_event(key(KeyCode::Char('h')));
    assert_eq!(app.editor.text(), "h");
    assert_eq!(app.focus, FocusPane::Editor);

    app.handle_key_event(key(KeyCode::Esc));
    assert!(!app.standard_editor_editing);
    app.handle_key_event(key(KeyCode::Char('h')));
    assert_eq!(app.editor.text(), "h");
    assert_eq!(app.focus, FocusPane::Explorer);
}

#[test]
fn mouse_clicking_tab_switches_view_without_staying_on_tabs() {
    let temp = tempfile::tempdir().expect("temp dir");
    let mut app = App::new(temp.path(), EditorMode::Standard).expect("app");
    let backend = TestBackend::new(120, 30);
    let mut terminal = Terminal::new(backend).expect("terminal");

    terminal.draw(|frame| app.render(frame)).expect("draw");
    let cfg_tab = app.layout.tab_hit_areas[2].1;
    app.handle_mouse_event(left_click(cfg_tab.x + 1, cfg_tab.y + 1));

    assert_eq!(app.active_tab, WorkbenchTab::Cfg);
    assert_eq!(app.focus, FocusPane::Editor);
}

#[test]
fn mouse_clicking_bytecode_tab_does_not_start_loading() {
    let temp = tempfile::tempdir().expect("temp dir");
    let mut app = App::new(temp.path(), EditorMode::Standard).expect("app");
    let backend = TestBackend::new(120, 30);
    let mut terminal = Terminal::new(backend).expect("terminal");

    terminal.draw(|frame| app.render(frame)).expect("draw");
    let bytecode_tab = app.layout.tab_hit_areas[1].1;
    app.handle_mouse_event(left_click(bytecode_tab.x + 1, bytecode_tab.y + 1));

    assert_eq!(app.active_tab, WorkbenchTab::Bytecode);
    assert!(matches!(app.bytecode, BytecodePane::Empty));
    assert!(app.bytecode_loader_rx.is_none());
    terminal.draw(|frame| app.render(frame)).expect("draw");
    let rendered = buffer_to_string(terminal.backend().buffer());
    assert!(!rendered.contains("Inspector"));
}

#[test]
fn mouse_clicking_editor_focuses_without_entering_edit_mode() {
    let temp = tempfile::tempdir().expect("temp dir");
    let mut app = App::new(temp.path(), EditorMode::Standard).expect("app");
    let backend = TestBackend::new(120, 30);
    let mut terminal = Terminal::new(backend).expect("terminal");

    terminal.draw(|frame| app.render(frame)).expect("draw");
    let editor = app.editor_text_area();
    app.handle_mouse_event(left_click(editor.x + 2, editor.y));
    app.handle_key_event(key(KeyCode::Char('j')));

    assert!(!app.standard_editor_editing);
    assert_eq!(app.editor.text(), "");
    assert_eq!(app.focus, FocusPane::Input);
}

#[test]
fn mouse_clicking_explorer_row_opens_file() {
    let temp = tempfile::tempdir().expect("temp dir");
    let file = temp.path().join("sample.move");
    fs::write(&file, "module sample::m {}").expect("write file");
    let mut app = App::new(temp.path(), EditorMode::Standard).expect("app");
    let backend = TestBackend::new(120, 30);
    let mut terminal = Terminal::new(backend).expect("terminal");

    terminal.draw(|frame| app.render(frame)).expect("draw");
    let explorer = inner_rect(app.layout.explorer);
    app.handle_mouse_event(left_click(explorer.x, explorer.y + 1));

    assert_eq!(app.focus, FocusPane::Editor);
    assert_eq!(app.editor.display_name(), "sample.move");
    assert_eq!(app.editor.text(), "module sample::m {}");
}

#[test]
fn mouse_scroll_pans_editor_vertically_and_horizontally() {
    let temp = tempfile::tempdir().expect("temp dir");
    let mut app = App::new(temp.path(), EditorMode::Standard).expect("app");
    app.focus = FocusPane::Editor;
    app.editor.lines = (0..24)
        .map(|index| format!("line {index:02} {}", "x".repeat(96)))
        .collect();

    let backend = TestBackend::new(100, 28);
    let mut terminal = Terminal::new(backend).expect("terminal");
    terminal.draw(|frame| app.render(frame)).expect("draw");
    let editor = app.editor_text_area();

    app.handle_mouse_event(scroll_down(editor.x, editor.y));
    app.handle_mouse_event(scroll_right(editor.x, editor.y));

    assert_eq!(app.focus, FocusPane::Editor);
    assert_eq!(app.editor.scroll, MOUSE_VERTICAL_SCROLL_STEP);
    assert_eq!(app.editor.horizontal_scroll, MOUSE_HORIZONTAL_SCROLL_STEP);

    app.handle_mouse_event(left_click(editor.x + 2, editor.y + 1));
    assert_eq!(app.editor.cursor.row, app.editor.scroll + 1);
    assert_eq!(app.editor.cursor.col, app.editor.horizontal_scroll + 2);
    assert!(!app.standard_editor_editing);
}

#[test]
fn mouse_scroll_explorer_moves_selection() {
    let temp = tempfile::tempdir().expect("temp dir");
    for index in 0..8 {
        fs::write(temp.path().join(format!("file-{index}.move")), "").expect("write file");
    }
    let mut app = App::new(temp.path(), EditorMode::Standard).expect("app");
    let backend = TestBackend::new(100, 28);
    let mut terminal = Terminal::new(backend).expect("terminal");
    terminal.draw(|frame| app.render(frame)).expect("draw");
    let explorer = inner_rect(app.layout.explorer);

    app.handle_mouse_event(scroll_down(explorer.x, explorer.y));

    assert_eq!(app.focus, FocusPane::Explorer);
    assert_eq!(app.explorer.selected(), MOUSE_VERTICAL_SCROLL_STEP);
}

#[test]
fn mouse_scroll_and_click_input_accounts_for_horizontal_offset() {
    let temp = tempfile::tempdir().expect("temp dir");
    let mut app = App::new(temp.path(), EditorMode::Standard).expect("app");
    app.input.text = "x".repeat(96);
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).expect("terminal");
    terminal.draw(|frame| app.render(frame)).expect("draw");
    let input = inner_rect(app.layout.input);

    app.handle_mouse_event(scroll_right(input.x, input.y));
    assert_eq!(app.focus, FocusPane::Input);
    assert_eq!(app.input.scroll, MOUSE_HORIZONTAL_SCROLL_STEP);

    app.handle_mouse_event(left_click(input.x + 3, input.y));
    assert_eq!(app.input.cursor, app.input.scroll + 3);
}

#[test]
fn editor_renders_line_numbers() {
    let temp = tempfile::tempdir().expect("temp dir");
    let mut app = App::new(temp.path(), EditorMode::Standard).expect("app");
    app.editor.lines = vec!["alpha".to_string(), "beta".to_string(), "gamma".to_string()];
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).expect("terminal");

    terminal.draw(|frame| app.render(frame)).expect("draw");
    let rendered = buffer_to_string(terminal.backend().buffer());

    assert!(rendered.contains(" 1 alpha"));
    assert!(rendered.contains(" 2 beta"));
    assert!(rendered.contains(" 3 gamma"));
}

#[test]
fn graph_loader_applies_background_result() {
    let temp = tempfile::tempdir().expect("temp dir");
    let mut app = App::new(temp.path(), EditorMode::Standard).expect("app");
    let expected = GraphDocument::new("type graph", "vault::Wallet".to_string());
    let (tx, rx) = mpsc::channel();
    app.graphs.set_loading(WorkbenchTab::TypeGraph);
    app.graph_loader_rx = Some((WorkbenchTab::TypeGraph, rx));
    tx.send(GraphLoadResult {
        tab: WorkbenchTab::TypeGraph,
        result: Ok(expected.clone()),
    })
    .expect("graph result");

    app.drain_graph_loader();

    let Some(GraphPane::Ready(document)) = app.graphs.get(WorkbenchTab::TypeGraph) else {
        panic!("expected ready graph document");
    };
    assert_eq!(document, &expected);
    assert!(app.graph_loader_rx.is_none());
}

#[test]
fn graph_tab_ready_document_scrolls_vertically_and_horizontally() {
    let temp = tempfile::tempdir().expect("temp dir");
    let mut app = App::new(temp.path(), EditorMode::Standard).expect("app");
    let text = (0..40)
        .map(|line| format!("line {line:02} {}", "x".repeat(120)))
        .collect::<Vec<_>>()
        .join("\n");
    app.set_active_tab(WorkbenchTab::TypeGraph);
    app.graphs.set_ready(
        WorkbenchTab::TypeGraph,
        GraphDocument::new("type graph", text),
    );

    let backend = TestBackend::new(100, 28);
    let mut terminal = Terminal::new(backend).expect("terminal");
    terminal.draw(|frame| app.render(frame)).expect("draw");
    let graph_area = inner_rect(app.layout.editor);

    app.handle_mouse_event(scroll_down(graph_area.x, graph_area.y));
    app.handle_mouse_event(scroll_right(graph_area.x, graph_area.y));

    let Some(GraphPane::Ready(document)) = app.graphs.get(WorkbenchTab::TypeGraph) else {
        panic!("expected ready graph document");
    };
    assert_eq!(document.scroll, MOUSE_VERTICAL_SCROLL_STEP);
    assert_eq!(document.horizontal_scroll, MOUSE_HORIZONTAL_SCROLL_STEP);
}

#[test]
fn type_graph_renderer_filters_modules_and_shows_edges() {
    let wallet = type_graph_node("type:struct:0x1::vault::Wallet", "vault", "Wallet", false);
    let balance = type_graph_node("builtin:type:u64", "", "u64", true);
    let other = type_graph_node("type:struct:0x1::other::Other", "other", "Other", false);
    let graph = MoveTypeGraph {
        nodes: vec![wallet, balance, other],
        edges: vec![MoveTypeGraphEdge {
            source: "type:struct:0x1::vault::Wallet".to_string(),
            target: "builtin:type:u64".to_string(),
            relationship: "has-field".to_string(),
            field_name: Some("balance".to_string()),
            variant_name: None,
            function_name: None,
            parameter_name: None,
            type_argument_index: None,
            is_mutable: false,
            is_reference: false,
            type_expression: Some("u64".to_string()),
            declaring_type_id: None,
            declaring_field_name: None,
            type_argument_name: None,
            source_spans: Vec::new(),
            confidence: "high".to_string(),
            evidence: Vec::new(),
        }],
        unresolved_types: vec![MoveUnresolvedType {
            source: "type:struct:0x1::vault::Wallet".to_string(),
            raw_type: "Unknown".to_string(),
            context: "field".to_string(),
            file_path: "sources/vault.move".to_string(),
            spans: Vec::new(),
            reason: "not resolved".to_string(),
        }],
    };

    let filtered = filter_type_graph(graph, &["vault".to_string()]);
    let rendered = render_type_graph_text(&filtered);

    assert!(rendered.contains("module 0x1::vault"));
    assert!(rendered.contains("struct Wallet"));
    assert!(rendered.contains("has-field field=balance type=u64 -> u64"));
    assert!(rendered.contains("unresolved Unknown in field: not resolved"));
    assert!(!rendered.contains("Other"));
}

#[test]
fn workbench_prefix_selects_tabs_and_toggles_mode() {
    let temp = tempfile::tempdir().expect("temp dir");
    let mut app = App::new(temp.path(), EditorMode::Standard).expect("app");
    app.focus = FocusPane::Editor;

    workbench_nav(&mut app, KeyCode::Char('3'));
    assert_eq!(app.focus, FocusPane::Editor);
    assert_eq!(app.active_tab, WorkbenchTab::Cfg);

    workbench_nav(&mut app, KeyCode::Char('m'));
    assert_eq!(app.editor_mode, EditorMode::Vim);
}

#[test]
fn global_tab_shortcuts_preserve_current_focus() {
    let temp = tempfile::tempdir().expect("temp dir");
    let mut app = App::new(temp.path(), EditorMode::Standard).expect("app");
    app.focus = FocusPane::Input;

    app.handle_key_event(key_with_modifiers(KeyCode::Char('4'), KeyModifiers::ALT));

    assert_eq!(app.active_tab, WorkbenchTab::CallGraph);
    assert_eq!(app.focus, FocusPane::Input);
}

#[test]
fn workbench_prefix_cycles_themes() {
    let temp = tempfile::tempdir().expect("temp dir");
    let mut app = App::new(temp.path(), EditorMode::Standard).expect("app");

    let original = app.theme.current_name();
    workbench_nav(&mut app, KeyCode::Char(']'));
    assert_eq!(app.theme.current_name(), original.next());
    assert!(app.status.contains("Theme:"));

    workbench_nav(&mut app, KeyCode::Char('['));
    assert_eq!(app.theme.current_name(), original);
}

#[test]
fn function_keys_are_not_workbench_navigation_shortcuts() {
    let temp = tempfile::tempdir().expect("temp dir");
    let mut app = App::new(temp.path(), EditorMode::Standard).expect("app");

    app.handle_key_event(key(KeyCode::F(3)));
    assert_eq!(app.focus, FocusPane::Explorer);

    app.handle_key_event(key(KeyCode::F(4)));
    assert_eq!(app.editor_mode, EditorMode::Standard);
}

#[test]
fn control_arrows_are_not_global_navigation_shortcuts() {
    let temp = tempfile::tempdir().expect("temp dir");
    let mut app = App::new(temp.path(), EditorMode::Standard).expect("app");

    app.handle_key_event(key_with_modifiers(KeyCode::Right, KeyModifiers::CONTROL));
    assert_eq!(app.focus, FocusPane::Explorer);
}

#[test]
fn global_quit_shortcuts_set_workbench_quit_exit() {
    let temp = tempfile::tempdir().expect("temp dir");
    let mut app = App::new(temp.path(), EditorMode::Standard).expect("app");

    app.handle_key_event(key_with_modifiers(
        KeyCode::Char('c'),
        KeyModifiers::CONTROL,
    ));
    assert_eq!(app.exit, Some(WorkbenchExit::Quit));

    app.exit = None;
    app.handle_key_event(key_with_modifiers(
        KeyCode::Char('q'),
        KeyModifiers::CONTROL,
    ));
    assert_eq!(app.exit, Some(WorkbenchExit::Quit));
}

#[test]
fn workbench_agent_command_switches_to_agent_exit() {
    let temp = tempfile::tempdir().expect("temp dir");
    let mut app = App::new(temp.path(), EditorMode::Standard).expect("app");

    app.focus = FocusPane::Input;
    app.input.text = ":agent".to_string();
    app.input.cursor = app.input.text.len();
    app.handle_key_event(key(KeyCode::Enter));

    assert_eq!(app.mode, AppMode::Agent);
    assert_eq!(app.exit, Some(WorkbenchExit::SwitchToAgent));
    assert_eq!(app.input.text, "");
}

#[test]
fn render_smoke_test_contains_workbench_regions() {
    let temp = tempfile::tempdir().expect("temp dir");
    fs::write(temp.path().join("Move.toml"), "[package]\n").expect("write file");
    let mut app = App::new(temp.path(), EditorMode::Standard).expect("app");
    let backend = TestBackend::new(120, 30);
    let mut terminal = Terminal::new(backend).expect("terminal");

    terminal.draw(|frame| app.render(frame)).expect("draw");
    let rendered = buffer_to_string(terminal.backend().buffer());

    assert!(rendered.contains("Explorer"));
    assert!(rendered.contains("code"));
    assert!(rendered.contains("▸"));
    assert!(rendered.contains("bytecode"));
    assert!(rendered.contains("call graph"));
    assert!(rendered.contains("Input"));
    assert!(rendered.contains("package: no status yet"));
    assert!(rendered.contains("standard"));
    assert!(!rendered.contains("app mode"));
    assert!(!rendered.contains("theme:"));
}

fn app_with_startup(
    root: &Path,
    trust_level: Option<peregrine_types::config_types::TrustLevel>,
) -> App {
    let mut app = App::new(root, EditorMode::Standard).expect("app");
    app.package_loader = fake_package_loader;
    app.created_package_trust_persister = trust_persister_ok;
    app.configure_launch_startup(test_trust_resolution(root, trust_level));
    app
}

fn test_trust_resolution(
    root: &Path,
    trust_level: Option<peregrine_types::config_types::TrustLevel>,
) -> WorkbenchTrustResolution {
    let cwd = root.canonicalize().expect("canonical root");
    let peregrine_home = cwd.join(".peregrine-home");
    fs::create_dir_all(&peregrine_home).expect("peregrine home");

    WorkbenchTrustResolution {
        cwd: cwd.clone(),
        trust_target: cwd,
        peregrine_home,
        trust_level,
    }
}

fn trust_persister_ok(project_root: &Path) -> Result<(), String> {
    if project_root.exists() {
        Ok(())
    } else {
        Err("project root does not exist".to_string())
    }
}

fn record_created_package_trust(project_root: &Path) -> Result<(), String> {
    CREATED_PACKAGE_TRUST_PERSISTED.store(true, AtomicOrdering::SeqCst);
    trust_persister_ok(project_root)
}

fn fake_package_loader(context: CliContext) -> PackageLoadReport {
    PackageLoadReport {
        package_root: context.package_root,
        build: passed_step("build"),
        test: passed_step("test"),
        scanners: PackageScannerReport {
            compiler_unit_tests: ScannerResult::NotFound,
            compiler_movy_invariant_tests: ScannerResult::NotFound,
            compiler_fuzz_tests: ScannerResult::NotFound,
            compiler_formal_verification: ScannerResult::NotFound,
            heuristic_unit_tests: ScannerResult::NotFound,
            heuristic_movy_invariant_tests: ScannerResult::NotFound,
            heuristic_fuzz_tests: ScannerResult::NotFound,
            heuristic_formal_verification: ScannerResult::NotFound,
        },
    }
}

fn passed_step(name: &str) -> CliStep {
    CliStep {
        name: name.to_string(),
        status: CliStatus::Passed,
        duration_ms: 0,
        exit_code: 0,
        command: None,
        diagnostics: Vec::new(),
        metadata: BTreeMap::new(),
        stdout: String::new(),
        stderr: String::new(),
        details: Value::Null,
    }
}

fn drain_startup_until_complete(app: &mut App) {
    for _ in 0..20 {
        app.drain_startup_task();
        if app.startup_task_rx.is_none() && app.package_load_report.is_some() {
            return;
        }
        thread::sleep(Duration::from_millis(10));
    }
}

fn lines_to_plain_text(lines: &[Line<'_>]) -> String {
    let mut text = String::new();
    for line in lines {
        for span in &line.spans {
            text.push_str(span.content.as_ref());
        }
        text.push('\n');
    }
    text
}

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn key_with_modifiers(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
    KeyEvent::new(code, modifiers)
}

fn left_click(column: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column,
        row,
        modifiers: KeyModifiers::NONE,
    }
}

fn scroll_down(column: u16, row: u16) -> MouseEvent {
    scroll_event(MouseEventKind::ScrollDown, column, row)
}

fn scroll_right(column: u16, row: u16) -> MouseEvent {
    scroll_event(MouseEventKind::ScrollRight, column, row)
}

fn scroll_event(kind: MouseEventKind, column: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind,
        column,
        row,
        modifiers: KeyModifiers::NONE,
    }
}

fn workbench_nav(app: &mut App, code: KeyCode) {
    app.handle_key_event(key_with_modifiers(
        KeyCode::Char('w'),
        KeyModifiers::CONTROL,
    ));
    app.handle_key_event(key(code));
}

fn type_graph_node(
    id: impl Into<String>,
    module_name: &str,
    name: &str,
    is_external: bool,
) -> MoveTypeGraphNode {
    let id = id.into();
    let module = (!module_name.is_empty()).then(|| module_name.to_string());
    let address = module.as_ref().map(|_| "0x1".to_string());
    let qualified_name = match (&address, &module) {
        (Some(address), Some(module)) => format!("{address}::{module}::{name}"),
        _ => name.to_string(),
    };

    MoveTypeGraphNode {
        id,
        kind: "struct".to_string(),
        package_name: Some("demo".to_string()),
        package_path: Some(".".to_string()),
        address,
        canonical_address: None,
        module_name: module,
        name: name.to_string(),
        qualified_name,
        file_path: None,
        abilities: Vec::new(),
        type_parameters: Vec::new(),
        attributes: Vec::new(),
        span: None,
        source: "source".to_string(),
        is_external,
    }
}

fn buffer_to_string(buffer: &ratatui::buffer::Buffer) -> String {
    let area = *buffer.area();
    let mut output = String::new();
    for y in area.y..area.y + area.height {
        for x in area.x..area.x + area.width {
            output.push_str(buffer[(x, y)].symbol());
        }
        output.push('\n');
    }
    output
}
