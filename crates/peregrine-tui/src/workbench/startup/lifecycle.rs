use crate::workbench::prelude::*;

use crate::chat;
use crate::keybinds;
use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread;

use crate::app;
use crate::navigation::Navigation;
use crate::sui::package_loader::{
    PackageCreateReport, PackageInspection, PackageLoadReport, WorkbenchTrustResolution,
    create_child_move_package, failed_create_report, inspect_package_directory,
    load_package_after_trust, persist_created_package_trust, resolve_trust_for_directory,
    trust_denied_load_report, workflow_failed_status,
};
use crate::sui::project::CliContext;
use crate::theme::{Theme, ThemeState};
use std::time::Instant;

use super::super::render_cli_step_summary;

impl App {
    pub fn from_current_dir() -> io::Result<Self> {
        let cwd = std::env::current_dir()?;
        let runtime = app::ApplicationRuntime::load(cwd.clone())?;
        Self::new_with_runtime(cwd, runtime)
    }

    pub fn from_launch_dir(root: Option<PathBuf>) -> io::Result<Self> {
        Self::from_launch_dir_with_async_runtime(root, None)
    }

    pub(crate) fn from_launch_dir_with_async_runtime(
        root: Option<PathBuf>,
        async_runtime: Option<std::sync::Arc<tokio::runtime::Runtime>>,
    ) -> io::Result<Self> {
        let root = match root {
            Some(root) => root,
            None => std::env::current_dir()?,
        };
        let runtime = match async_runtime {
            Some(async_runtime) => {
                app::ApplicationRuntime::load_with_async_runtime(
                    root.clone(),
                    /*peregrine_home*/ None,
                    async_runtime,
                )?
            }
            None => app::ApplicationRuntime::load(root.clone())?,
        };
        let trust_resolution = resolve_trust_for_directory(&root)?;
        let mut app = Self::new_with_runtime(root, runtime)?;
        app.configure_launch_startup(trust_resolution);
        Ok(app)
    }

    pub fn new(root: impl AsRef<Path>, editor_mode: EditorMode) -> io::Result<Self> {
        Self::new_with_theme(root, editor_mode, Theme::default())
    }

    pub fn new_with_theme(
        root: impl AsRef<Path>,
        editor_mode: EditorMode,
        theme: Theme,
    ) -> io::Result<Self> {
        Self::new_with_theme_state(root, editor_mode, ThemeState::new(theme))
    }

    pub(crate) fn new_with_theme_state(
        root: impl AsRef<Path>,
        editor_mode: EditorMode,
        theme: ThemeState,
    ) -> io::Result<Self> {
        Self::new_with_parts(root, editor_mode, theme, None)
    }

    pub(crate) fn new_with_runtime(
        root: impl AsRef<Path>,
        runtime: app::ApplicationRuntime,
    ) -> io::Result<Self> {
        let ui = runtime.ui();
        Self::new_with_parts(root, ui.editor_mode, runtime.theme(), Some(runtime))
    }

    pub(crate) fn new_with_parts(
        root: impl AsRef<Path>,
        editor_mode: EditorMode,
        theme: ThemeState,
        application_runtime: Option<app::ApplicationRuntime>,
    ) -> io::Result<Self> {
        keybinds::init_default_keybindings()?;
        let root = root.as_ref().to_path_buf();
        let theme_generation = theme.generation();
        let application_config = application_runtime
            .as_ref()
            .map(app::ApplicationRuntime::config);
        let chat = application_config
            .as_deref()
            .cloned()
            .zip(application_runtime.clone())
            .map(|(config, runtime)| chat::ChatController::new(config, runtime))
            .unwrap_or_default();
        let app = Self {
            application_runtime,
            application_config,
            mode: AppMode::default(),
            focus: FocusPane::Explorer,
            active_tab: WorkbenchTab::Editor,
            editor_mode,
            standard_editor_editing: false,
            vim_state: VimState::Normal,
            theme,
            theme_generation,
            navigation: Navigation::default(),
            explorer: Explorer::new(&root)?,
            editor: EditorWorkspace::new(&root),
            editor_render_cache: None,
            pending_close: None,
            bytecode: BytecodePane::default(),
            bytecode_cache: HashMap::new(),
            bytecode_loader_rx: None,
            bytecode_load_epoch: 0,
            graphs: GraphPanes::default(),
            graph_loader_rx: None,
            chat,
            startup: WorkbenchStartupState::Workbench,
            startup_task_rx: None,
            package_load_report: None,
            created_package_trust_persister: persist_created_package_trust,
            package_loader: load_package_after_trust,
            exit: None,
            status: keybinds::default_hint(),
            layout: WorkbenchLayout::default(),
        };
        app.sync_syntax_theme();
        Ok(app)
    }

    pub(crate) fn configure_launch_startup(&mut self, trust_resolution: WorkbenchTrustResolution) {
        match inspect_package_directory(&self.explorer.root) {
            PackageInspection::Valid { context } => {
                self.apply_trust_resolution(trust_resolution, TrustPostAction::LoadPackage(context))
            }
            PackageInspection::Invalid { root, message } => {
                self.status =
                    "Selected directory is not a valid Move package; choose how to continue"
                        .to_string();
                self.startup = WorkbenchStartupState::InvalidPackageChoice(InvalidPackagePrompt {
                    root,
                    message,
                    trust_resolution,
                    selected: InvalidPackageAction::CreatePackage,
                });
            }
        }
    }

    pub(crate) fn apply_trust_resolution(
        &mut self,
        resolution: WorkbenchTrustResolution,
        post_action: TrustPostAction,
    ) {
        if resolution.is_trusted() {
            self.run_post_trust_action(post_action);
            return;
        }

        if resolution.is_untrusted() {
            self.handle_trust_denied(
                post_action,
                "Project is marked untrusted; build, tests, and scanners were skipped.".to_string(),
            );
            return;
        }

        self.status = "Awaiting project trust decision".to_string();
        self.startup = WorkbenchStartupState::TrustDecision(TrustPrompt {
            resolution,
            post_action,
            selected: TrustAction::Trust,
            error: None,
        });
    }

    pub(crate) fn run_post_trust_action(&mut self, post_action: TrustPostAction) {
        match post_action {
            TrustPostAction::EnterWorkbench => {
                self.startup = WorkbenchStartupState::Workbench;
                self.status = keybinds::default_hint();
            }
            TrustPostAction::LoadPackage(context) => self.start_package_load(context),
        }
    }

    pub(crate) fn handle_trust_denied(&mut self, post_action: TrustPostAction, message: String) {
        match post_action {
            TrustPostAction::EnterWorkbench => {
                self.startup = WorkbenchStartupState::Workbench;
                self.status = message;
            }
            TrustPostAction::LoadPackage(context) => {
                let report = trust_denied_load_report(context.package_root, message);
                self.status = "Project trust denied; package load skipped".to_string();
                self.finish_package_load(report);
            }
        }
    }

    pub(crate) fn start_package_load(&mut self, context: CliContext) {
        let package_root = context.package_root.clone();
        let (tx, rx) = mpsc::channel();
        let package_loader = self.package_loader;

        match thread::Builder::new()
            .name("peregrine-package-load".to_string())
            .spawn(move || {
                let report = package_loader(context);
                let _ = tx.send(StartupTaskResult::LoadPackage(report));
            }) {
            Ok(_) => {
                self.startup_task_rx = Some(rx);
                self.package_load_report = None;
                self.startup = WorkbenchStartupState::PackageLoadRunning(PackageLoadRunningState {
                    message: format!("Building, testing, and scanning {}", package_root.display()),
                    started_at: Instant::now(),
                });
                self.status = "Package load running".to_string();
            }
            Err(error) => {
                let report = startup_failure_load_report(
                    package_root,
                    format!("Could not start package loader: {error}"),
                );
                self.status = "Package load failed to start".to_string();
                self.finish_package_load(report);
            }
        }
    }

    pub(crate) fn start_create_package(
        &mut self,
        parent: PathBuf,
        package_name: String,
        trust_resolution: WorkbenchTrustResolution,
        invalid_message: String,
    ) {
        let (tx, rx) = mpsc::channel();
        let thread_parent = parent.clone();
        let thread_package_name = package_name.clone();
        let thread_trust_resolution = trust_resolution.clone();
        let thread_invalid_message = invalid_message.clone();

        match thread::Builder::new()
            .name("peregrine-package-create".to_string())
            .spawn(move || {
                let report = create_child_move_package(&thread_parent, &thread_package_name);
                let _ = tx.send(StartupTaskResult::CreatePackage {
                    parent: thread_parent,
                    package_name: thread_package_name,
                    trust_resolution: thread_trust_resolution,
                    invalid_message: thread_invalid_message,
                    report,
                });
            }) {
            Ok(_) => {
                self.startup_task_rx = Some(rx);
                self.package_load_report = None;
                self.startup = WorkbenchStartupState::PackageLoadRunning(PackageLoadRunningState {
                    message: format!("Creating Move package `{package_name}`"),
                    started_at: Instant::now(),
                });
                self.status = format!("Creating Move package `{package_name}`");
            }
            Err(error) => {
                let report = failed_create_report(
                    &parent,
                    &package_name,
                    format!("Could not start package creation: {error}"),
                );
                self.apply_create_package_report(
                    parent,
                    package_name,
                    trust_resolution,
                    invalid_message,
                    report,
                );
            }
        }
    }

    pub(crate) fn drain_startup_task(&mut self) {
        let event = match self.startup_task_rx.as_ref() {
            Some(rx) => match rx.try_recv() {
                Ok(result) => Some(Ok(result)),
                Err(mpsc::TryRecvError::Empty) => None,
                Err(mpsc::TryRecvError::Disconnected) => Some(Err(
                    "Startup worker stopped before returning a result.".to_string(),
                )),
            },
            None => None,
        };

        match event {
            Some(Ok(StartupTaskResult::CreatePackage {
                parent,
                package_name,
                trust_resolution,
                invalid_message,
                report,
            })) => {
                self.startup_task_rx = None;
                self.apply_create_package_report(
                    parent,
                    package_name,
                    trust_resolution,
                    invalid_message,
                    report,
                );
            }
            Some(Ok(StartupTaskResult::LoadPackage(report))) => {
                self.startup_task_rx = None;
                self.finish_package_load(report);
            }
            Some(Err(message)) => {
                self.startup_task_rx = None;
                let package_root = self.explorer.root.clone();
                let report = startup_failure_load_report(package_root, message);
                self.finish_package_load(report);
            }
            None => {}
        }
    }

    pub(crate) fn finish_package_load(&mut self, report: PackageLoadReport) {
        self.status = package_load_status(&report);
        self.package_load_report = Some(report);
        self.startup = WorkbenchStartupState::Workbench;
    }

    pub(crate) fn apply_create_package_report(
        &mut self,
        parent: PathBuf,
        package_name: String,
        trust_resolution: WorkbenchTrustResolution,
        invalid_message: String,
        report: PackageCreateReport,
    ) {
        if workflow_failed_status(&report.step) {
            let error = render_cli_step_summary(&report.step);
            self.status = format!("Package creation failed: {error}");
            self.startup = WorkbenchStartupState::PackageNameEntry(PackageNamePrompt {
                parent,
                input: CommandInput::from_text(package_name),
                error: Some(error),
                trust_resolution,
                invalid_message,
            });
            return;
        }

        if let Err(message) = self.switch_workbench_root(report.package_root.as_path()) {
            let report = startup_failure_load_report(report.package_root, message);
            self.finish_package_load(report);
            return;
        }

        let package_root = self.explorer.root.clone();
        match (self.created_package_trust_persister)(package_root.as_path()) {
            Ok(()) => {
                self.status = format!("Created {} and marked it trusted", package_root.display());
            }
            Err(error) => {
                self.status = format!("Created package; trust persistence failed: {error}");
            }
        }

        match inspect_package_directory(package_root.as_path()) {
            PackageInspection::Valid { context } => self.start_package_load(context),
            PackageInspection::Invalid { message, .. } => {
                let report = startup_failure_load_report(package_root, message);
                self.finish_package_load(report);
            }
        }
    }

    pub(crate) fn switch_workbench_root(&mut self, root: &Path) -> Result<(), String> {
        self.explorer =
            Explorer::new(root).map_err(|error| format!("Could not open project root: {error}"))?;
        self.editor = EditorWorkspace::new(root);
        self.pending_close = None;
        self.chat.shutdown();
        self.chat = self
            .application_config
            .as_deref()
            .cloned()
            .zip(self.application_runtime.clone())
            .map(|(config, runtime)| chat::ChatController::new(config, runtime))
            .unwrap_or_default();
        self.focus = FocusPane::Explorer;
        self.active_tab = WorkbenchTab::Editor;
        self.standard_editor_editing = false;
        self.vim_state = VimState::Normal;
        self.package_load_report = None;
        self.invalidate_workbench_views();
        Ok(())
    }
}
