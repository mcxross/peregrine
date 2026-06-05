mod agent;
mod args;
pub mod helper_args;
mod keybinds;
mod navigation;
mod output;
pub mod sui;
pub mod tabs;
pub mod theme;
mod workbench_render;
mod workflow;

use crate::navigation::{Navigation, NavigationCommand, NavigationIntent};
use crate::output::{CliStatus, CliStep};
use crate::sui::args::{CallGraphArgs, CfgArgs, GraphOutputArgs};
use crate::sui::package_loader::{
    PackageCreateReport, PackageInspection, PackageLoadReport, PackageScannerReport, ScannerResult,
    WorkbenchTrustResolution, create_child_move_package, failed_create_report, failed_startup_step,
    inspect_package_directory, load_package_after_trust, persist_created_package_trust,
    persist_trust_for_resolution, resolve_trust_for_directory, trust_denied_load_report,
    workflow_failed_status,
};
use crate::sui::project::{BytecodeTarget, CliContext, bytecode_targets, resolve_context};
use crate::sui::runners::{run_call_graph, run_cfg};
use crate::tabs::{TabNav, tab_hit_areas};
use crate::theme::{Theme, ThemeName, ThemePalette};
use crate::workbench_render::{
    RenderedWorkbenchDocument, is_markdown_path, render_workbench_document,
};
use clap::Parser;
use codex_arg0::Arg0DispatchPaths;
use codex_utils_cli::CliConfigOverrides;
use move_binary_format::file_format::{CodeOffset, CompiledModule, FunctionDefinitionIndex};
use move_bytecode_source_map::{mapping::SourceMapping, source_map::SourceMap};
use move_disassembler::disassembler::{Disassembler, DisassemblerOptions};
use peregrine_config::config_toml::ConfigToml;
use peregrine_config::{CONFIG_TOML_FILE, LoaderOverrides};
use peregrine_move_graphs::{
    MoveTypeGraph, MoveTypeGraphEdge, MoveTypeGraphNode, MoveUnresolvedType,
    discover_move_project_graphs_for_package,
};
use ratatui::crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyEventKind,
    KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::crossterm::execute;
use ratatui::layout::{Constraint, Direction, Layout, Position, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::{DefaultTerminal, Frame};
use regex::Regex;
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant, UNIX_EPOCH};

const UNDO_LIMIT: usize = 100;
const AGENT_TOKIO_WORKER_STACK_SIZE_BYTES: usize = 16 * 1024 * 1024;
const PAGE_SIZE: usize = 12;
const MOUSE_VERTICAL_SCROLL_STEP: usize = 3;
const MOUSE_HORIZONTAL_SCROLL_STEP: usize = 8;
const AGENT_SUBCOMMAND: &str = "agent";
const WORKBENCH_TAB_LABELS: [&str; 5] = ["code", "bytecode", "cfg", "call graph", "type graph"];
const CLI_COMMAND_NAMES: &[&str] = &[
    "build",
    "test",
    "coverage",
    "bytecode",
    "bytecode-viewer",
    "signatures",
    "function-signatures",
    "call-graph",
    "callgraph",
    "object-graph",
    "objectgraph",
    "cfg",
    "control-flow-graph",
    "fuzz",
    "verify",
    "analyze",
    "check-all",
    "import-package",
    "new-package",
];

pub fn run() -> io::Result<i32> {
    run_from_env_args(std::env::args_os())
}

pub fn run_from_env_args<I>(args: I) -> io::Result<i32>
where
    I: IntoIterator<Item = OsString>,
{
    let mut args = args.into_iter();
    let _binary = args.next();
    let args = args.collect::<Vec<_>>();

    match classify_top_level_args(args) {
        TopLevelDispatch::Workbench(root) => run_mode_shell(root),
        TopLevelDispatch::Agent(args) => match run_agent_from_args(args)? {
            AgentExit::Quit(code) => Ok(code),
            AgentExit::SwitchToWorkbench => run_mode_shell(None),
        },
        TopLevelDispatch::CliOrHelper(args) => Ok(run_cli_or_helper_from_args(args)),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum TopLevelDispatch {
    Workbench(Option<PathBuf>),
    Agent(Vec<OsString>),
    CliOrHelper(Vec<OsString>),
}

fn classify_top_level_args(args: Vec<OsString>) -> TopLevelDispatch {
    if args
        .first()
        .is_some_and(|arg| arg == OsStr::new(AGENT_SUBCOMMAND))
    {
        return TopLevelDispatch::Agent(args.into_iter().skip(1).collect());
    }

    if args.is_empty() {
        return TopLevelDispatch::Workbench(None);
    }

    if let [first] = args.as_slice() {
        if is_helper_arg(first) || is_cli_command_arg(first) || is_flag_arg(first) {
            return TopLevelDispatch::CliOrHelper(args);
        }

        return TopLevelDispatch::Workbench(Some(PathBuf::from(first)));
    }

    TopLevelDispatch::CliOrHelper(args)
}

fn is_helper_arg(arg: &OsString) -> bool {
    arg.as_os_str() == OsStr::new(helper_args::BUNDLED_SUI_HELPER_ARG)
        || arg.as_os_str() == OsStr::new(helper_args::BYTECODE_VIEWER_HELPER_ARG)
        || arg.as_os_str() == OsStr::new(helper_args::MOVY_FUZZ_HELPER_ARG)
        || arg.as_os_str() == OsStr::new(helper_args::FORMAL_VERIFICATION_HELPER_ARG)
        || arg.as_os_str() == OsStr::new(helper_args::MOVE_ANALYZER_HELPER_ARG)
}

fn is_cli_command_arg(arg: &OsString) -> bool {
    arg.to_str()
        .is_some_and(|arg| CLI_COMMAND_NAMES.contains(&arg))
}

fn is_flag_arg(arg: &OsString) -> bool {
    arg.to_str().is_some_and(|arg| arg.starts_with('-'))
}

fn run_mode_shell(root: Option<PathBuf>) -> io::Result<i32> {
    let mut app = App::from_launch_dir(root)?;
    loop {
        match run_tui(&mut app)? {
            WorkbenchExit::Quit => return Ok(0),
            WorkbenchExit::SwitchToAgent => match run_agent_from_args(std::iter::empty())? {
                AgentExit::Quit(code) => return Ok(code),
                AgentExit::SwitchToWorkbench => {
                    app = App::from_launch_dir(None)?;
                }
            },
        }
    }
}

pub fn run_tui(app: &mut App) -> io::Result<WorkbenchExit> {
    let mut terminal = ratatui::try_init()?;
    let mut terminal_guard = WorkbenchTerminalGuard::new();
    if let Err(error) = execute!(io::stdout(), EnableMouseCapture) {
        let _ = terminal_guard.restore();
        return Err(error);
    }
    terminal_guard.mouse_capture_enabled = true;

    let result = app.run(&mut terminal);
    let cleanup_result = terminal_guard.restore();

    match (result, cleanup_result) {
        (Ok(exit), Ok(())) => Ok(exit),
        (Err(error), _) => Err(error),
        (Ok(_), Err(error)) => Err(error),
    }
}

struct WorkbenchTerminalGuard {
    active: bool,
    mouse_capture_enabled: bool,
}

impl WorkbenchTerminalGuard {
    fn new() -> Self {
        Self {
            active: true,
            mouse_capture_enabled: false,
        }
    }

    fn restore(&mut self) -> io::Result<()> {
        if !self.active {
            return Ok(());
        }

        let mouse_result = if self.mouse_capture_enabled {
            execute!(io::stdout(), DisableMouseCapture)
        } else {
            Ok(())
        };
        ratatui::restore();
        self.active = false;
        mouse_result
    }
}

impl Drop for WorkbenchTerminalGuard {
    fn drop(&mut self) {
        let _ = self.restore();
    }
}

fn run_agent_from_args<I>(args: I) -> io::Result<AgentExit>
where
    I: IntoIterator<Item = OsString>,
{
    let top_cli = match AgentTopCli::try_parse_from(
        std::iter::once(OsString::from("peregrine-tui agent")).chain(args),
    ) {
        Ok(cli) => cli,
        Err(error) => {
            let exit_code = error.exit_code();
            let _ = error.print();
            return Ok(AgentExit::Quit(exit_code));
        }
    };
    let mut inner = top_cli.inner;
    inner
        .config_overrides
        .raw_overrides
        .splice(0..0, top_cli.config_overrides.raw_overrides);

    let runtime = build_agent_runtime()?;
    let exit_info = runtime.block_on(agent::run_main(
        inner,
        agent_arg0_dispatch_paths()?,
        LoaderOverrides::default(),
        /*explicit_remote_endpoint*/ None,
    ))?;

    match exit_info.exit_reason {
        agent::ExitReason::SwitchToWorkbench => Ok(AgentExit::SwitchToWorkbench),
        agent::ExitReason::UserRequested => Ok(AgentExit::Quit(0)),
        agent::ExitReason::Fatal(message) => {
            eprintln!("ERROR: {message}");
            Ok(AgentExit::Quit(1))
        }
    }
}

fn build_agent_runtime() -> io::Result<tokio::runtime::Runtime> {
    let mut builder = tokio::runtime::Builder::new_multi_thread();
    builder.enable_all();
    // Match the upstream CLI runtime. The embedded app-server has large debug async
    // state machines, and Tokio's default worker stack can overflow on startup.
    builder.thread_stack_size(AGENT_TOKIO_WORKER_STACK_SIZE_BYTES);
    builder.build().map_err(io::Error::other)
}

fn agent_arg0_dispatch_paths() -> io::Result<Arg0DispatchPaths> {
    Ok(Arg0DispatchPaths {
        codex_self_exe: Some(std::env::current_exe()?),
        codex_linux_sandbox_exe: None,
        main_execve_wrapper_exe: None,
    })
}

#[derive(Parser, Debug)]
struct AgentTopCli {
    #[clap(flatten)]
    config_overrides: CliConfigOverrides,

    #[clap(flatten)]
    inner: agent::Cli,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AgentExit {
    Quit(i32),
    SwitchToWorkbench,
}

pub fn run_cli_or_helper_from_args<I>(args: I) -> i32
where
    I: IntoIterator<Item = OsString>,
{
    let mut args = args.into_iter();

    match args.next() {
        Some(arg) if arg.as_os_str() == OsStr::new(helper_args::BUNDLED_SUI_HELPER_ARG) => {
            run_bundled_sui_helper(args);
        }
        Some(arg) if arg.as_os_str() == OsStr::new(helper_args::BYTECODE_VIEWER_HELPER_ARG) => {
            run_bytecode_viewer_helper(args);
        }
        Some(arg) if arg.as_os_str() == OsStr::new(helper_args::MOVY_FUZZ_HELPER_ARG) => {
            run_movy_fuzz_helper(args);
        }
        Some(arg) if arg.as_os_str() == OsStr::new(helper_args::FORMAL_VERIFICATION_HELPER_ARG) => {
            run_formal_verification_helper(args);
        }
        Some(arg) if arg.as_os_str() == OsStr::new(helper_args::MOVE_ANALYZER_HELPER_ARG) => {
            run_move_analyzer_helper();
        }
        Some(arg) => run_cli_from_args(std::iter::once(arg).chain(args)),
        None => run_cli_from_args(std::iter::empty()),
    }
}

pub fn run_cli_from_args<I>(args: I) -> i32
where
    I: IntoIterator<Item = OsString>,
{
    let cli =
        match args::Cli::try_parse_from(std::iter::once(OsString::from("peregrine")).chain(args)) {
            Ok(cli) => cli,
            Err(error) => {
                let exit_code = error.exit_code();
                let _ = error.print();
                return exit_code;
            }
        };
    let json = cli.json;
    let report = workflow::execute(&cli);
    let exit_code = report.exit_code;

    if let Err(error) = output::write_report(&report, json) {
        eprintln!("{error}");
        return output::EXIT_USAGE;
    }

    exit_code
}

fn run_bundled_sui_helper(args: impl IntoIterator<Item = OsString>) -> ! {
    match peregrine_adapters::sui::run_bundled_sui_blocking(args) {
        Ok(output) => {
            print!("{}", output.stdout);
            eprint!("{}", output.stderr);
            std::process::exit(output.status.unwrap_or(1));
        }
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}

fn run_move_analyzer_helper() -> ! {
    peregrine_adapters::move_analyzer::run_bundled_move_analyzer_stdio();
    std::process::exit(0);
}

fn run_bytecode_viewer_helper(mut args: impl Iterator<Item = OsString>) -> ! {
    let Some(package_root) = args.next() else {
        eprintln!("missing package root");
        std::process::exit(1);
    };
    let Some(module_name) = args.next() else {
        eprintln!("missing module name");
        std::process::exit(1);
    };
    let mut interactive = false;
    let mut bytecode_map = false;
    let mut debug = false;

    for arg in args {
        match arg.to_string_lossy().as_ref() {
            "--interactive" => interactive = true,
            "--bytecode-map" => bytecode_map = true,
            "--debug" => debug = true,
            unknown => {
                eprintln!("unknown bytecode viewer option: {unknown}");
                std::process::exit(1);
            }
        }
    }

    let package_root = PathBuf::from(package_root);
    let module_name = module_name.to_string_lossy().into_owned();
    let install_dir = tempfile::tempdir().expect("bytecode viewer install dir");
    let mut build_config = move_package_alt_compilation::build_config::BuildConfig::default();
    build_config.install_dir = Some(install_dir.path().to_path_buf());
    let disassemble = move_cli::base::disassemble::Disassemble {
        interactive,
        package_name: None,
        module_or_script_name: module_name,
        debug,
        bytecode_map,
    };
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("bytecode viewer runtime");
    let result = runtime.block_on(
        disassemble
            .execute::<sui_package_alt::SuiFlavor>(Some(package_root.as_path()), build_config),
    );
    if interactive {
        restore_bytecode_viewer_terminal();
    }

    match result {
        Ok(()) => std::process::exit(0),
        Err(error) => {
            eprintln!("{error:#}");
            std::process::exit(1);
        }
    }
}

fn restore_bytecode_viewer_terminal() {
    let mut stdout = std::io::stdout();
    let _ = crossterm::execute!(
        stdout,
        crossterm::event::DisableMouseCapture,
        crossterm::terminal::LeaveAlternateScreen
    );
    let _ = crossterm::terminal::disable_raw_mode();
}

fn run_movy_fuzz_helper(mut args: impl Iterator<Item = OsString>) -> ! {
    let Some(root_path) = args.next() else {
        eprintln!("missing root path");
        std::process::exit(1);
    };
    let Some(package_path) = args.next() else {
        eprintln!("missing package path");
        std::process::exit(1);
    };
    let time_limit_seconds = args
        .next()
        .and_then(|value| value.to_string_lossy().parse::<u64>().ok())
        .unwrap_or(30);
    let seed = args
        .next()
        .and_then(|value| value.to_string_lossy().parse::<u64>().ok())
        .unwrap_or(1);

    let package_path = package_path.to_string_lossy().into_owned();
    match peregrine_dynamic_analysis::sui::movy_fuzz::run_movy_fuzz_blocking(
        PathBuf::from(root_path),
        &package_path,
        peregrine_dynamic_analysis::sui::movy_fuzz::MovyFuzzOptions {
            time_limit_seconds,
            seed,
        },
    ) {
        Ok(run) => {
            println!("{}", run.stdout);
            std::process::exit(0);
        }
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}

fn run_formal_verification_helper(mut args: impl Iterator<Item = OsString>) -> ! {
    let Some(root_path) = args.next() else {
        eprintln!("missing root path");
        std::process::exit(1);
    };
    let Some(package_path) = args.next() else {
        eprintln!("missing package path");
        std::process::exit(1);
    };
    let Some(file_path) = args.next() else {
        eprintln!("missing file path");
        std::process::exit(1);
    };
    let Some(module_name) = args.next() else {
        eprintln!("missing module name");
        std::process::exit(1);
    };
    let timeout_seconds = args
        .next()
        .and_then(|value| value.to_string_lossy().parse::<usize>().ok());

    let package_path = package_path.to_string_lossy().into_owned();
    match peregrine_dynamic_analysis::sui::formal_verification::run_formal_verification_blocking(
        PathBuf::from(root_path),
        &package_path,
        peregrine_dynamic_analysis::sui::formal_verification::FormalVerificationOptions {
            file_path: file_path.to_string_lossy().into_owned(),
            module_name: module_name.to_string_lossy().into_owned(),
            timeout_seconds,
            verbose: true,
            trace: false,
            keep_temp: false,
        },
    ) {
        Ok(_) => std::process::exit(0),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppMode {
    Workbench,
    Agent,
}

impl Default for AppMode {
    fn default() -> Self {
        Self::Workbench
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusPane {
    Explorer,
    Tabs,
    Editor,
    Input,
    Inspector,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkbenchTab {
    Code,
    Bytecode,
    Cfg,
    CallGraph,
    TypeGraph,
}

impl WorkbenchTab {
    const ALL: [Self; 5] = [
        Self::Code,
        Self::Bytecode,
        Self::Cfg,
        Self::CallGraph,
        Self::TypeGraph,
    ];

    fn title(self) -> &'static str {
        match self {
            Self::Code => "code",
            Self::Bytecode => "bytecode",
            Self::Cfg => "cfg",
            Self::CallGraph => "call graph",
            Self::TypeGraph => "type graph",
        }
    }

    fn index(self) -> usize {
        Self::ALL
            .iter()
            .position(|tab| *tab == self)
            .unwrap_or_default()
    }
}

impl Default for WorkbenchTab {
    fn default() -> Self {
        Self::Code
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorMode {
    Standard,
    Vim,
}

impl Default for EditorMode {
    fn default() -> Self {
        Self::Standard
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VimState {
    Normal,
    Insert,
}

impl Default for VimState {
    fn default() -> Self {
        Self::Normal
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkbenchExit {
    Quit,
    SwitchToAgent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InvalidPackageAction {
    CreatePackage,
    ProceedAnyway,
}

impl InvalidPackageAction {
    fn toggle(self) -> Self {
        match self {
            Self::CreatePackage => Self::ProceedAnyway,
            Self::ProceedAnyway => Self::CreatePackage,
        }
    }
}

#[derive(Debug, Clone)]
struct InvalidPackagePrompt {
    root: PathBuf,
    message: String,
    trust_resolution: WorkbenchTrustResolution,
    selected: InvalidPackageAction,
}

#[derive(Debug, Clone)]
struct PackageNamePrompt {
    parent: PathBuf,
    input: CommandInput,
    error: Option<String>,
    trust_resolution: WorkbenchTrustResolution,
    invalid_message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TrustAction {
    Trust,
    ContinueWithoutTrust,
}

impl TrustAction {
    fn toggle(self) -> Self {
        match self {
            Self::Trust => Self::ContinueWithoutTrust,
            Self::ContinueWithoutTrust => Self::Trust,
        }
    }
}

#[derive(Debug, Clone)]
struct TrustPrompt {
    resolution: WorkbenchTrustResolution,
    post_action: TrustPostAction,
    selected: TrustAction,
    error: Option<String>,
}

#[derive(Debug, Clone)]
enum TrustPostAction {
    EnterWorkbench,
    LoadPackage(CliContext),
}

#[derive(Debug, Clone)]
struct PackageLoadRunningState {
    message: String,
    started_at: Instant,
}

#[derive(Debug, Clone)]
enum WorkbenchStartupState {
    Workbench,
    InvalidPackageChoice(InvalidPackagePrompt),
    PackageNameEntry(PackageNamePrompt),
    TrustDecision(TrustPrompt),
    PackageLoadRunning(PackageLoadRunningState),
}

impl WorkbenchStartupState {
    fn is_workbench(&self) -> bool {
        matches!(self, Self::Workbench | Self::PackageLoadRunning(_))
    }
}

enum StartupTaskResult {
    CreatePackage {
        parent: PathBuf,
        package_name: String,
        trust_resolution: WorkbenchTrustResolution,
        invalid_message: String,
        report: PackageCreateReport,
    },
    LoadPackage(PackageLoadReport),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScrollDirection {
    Up,
    Down,
    Left,
    Right,
}

#[derive(Debug, Default, Clone)]
struct WorkbenchLayout {
    explorer: Rect,
    tabs: Rect,
    tab_hit_areas: Vec<(WorkbenchTab, Rect)>,
    editor: Rect,
    input: Rect,
    inspector: Option<Rect>,
}

pub struct App {
    mode: AppMode,
    focus: FocusPane,
    active_tab: WorkbenchTab,
    editor_mode: EditorMode,
    standard_editor_editing: bool,
    vim_state: VimState,
    theme: Theme,
    navigation: Navigation,
    explorer: Explorer,
    editor: EditorBuffer,
    editor_render_cache: Option<EditorRenderCache>,
    bytecode: BytecodePane,
    bytecode_cache: HashMap<BytecodeTargetKey, BytecodeCacheEntry>,
    bytecode_loader_rx: Option<mpsc::Receiver<BytecodeLoadResult>>,
    bytecode_load_epoch: u64,
    graphs: GraphPanes,
    input: CommandInput,
    startup: WorkbenchStartupState,
    startup_task_rx: Option<mpsc::Receiver<StartupTaskResult>>,
    package_load_report: Option<PackageLoadReport>,
    created_package_trust_persister: fn(&Path) -> Result<(), String>,
    package_loader: fn(CliContext) -> PackageLoadReport,
    exit: Option<WorkbenchExit>,
    status: String,
    layout: WorkbenchLayout,
}

impl App {
    pub fn from_current_dir() -> io::Result<Self> {
        let cwd = std::env::current_dir()?;
        let settings = configured_tui_settings();
        Self::new_with_theme(cwd, settings.editor_mode, settings.theme)
    }

    pub fn from_launch_dir(root: Option<PathBuf>) -> io::Result<Self> {
        let root = match root {
            Some(root) => root,
            None => std::env::current_dir()?,
        };
        let settings = configured_tui_settings();
        let trust_resolution = resolve_trust_for_directory(&root)?;
        let mut app = Self::new_with_theme(root, settings.editor_mode, settings.theme)?;
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
        keybinds::init_default_keybindings()?;
        let app = Self {
            mode: AppMode::default(),
            focus: FocusPane::Explorer,
            active_tab: WorkbenchTab::Code,
            editor_mode,
            standard_editor_editing: false,
            vim_state: VimState::Normal,
            theme,
            navigation: Navigation::default(),
            explorer: Explorer::new(root)?,
            editor: EditorBuffer::new_empty(),
            editor_render_cache: None,
            bytecode: BytecodePane::default(),
            bytecode_cache: HashMap::new(),
            bytecode_loader_rx: None,
            bytecode_load_epoch: 0,
            graphs: GraphPanes::default(),
            input: CommandInput::default(),
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

    fn configure_launch_startup(&mut self, trust_resolution: WorkbenchTrustResolution) {
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

    fn apply_trust_resolution(
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

    fn run_post_trust_action(&mut self, post_action: TrustPostAction) {
        match post_action {
            TrustPostAction::EnterWorkbench => {
                self.startup = WorkbenchStartupState::Workbench;
                self.status = keybinds::default_hint();
            }
            TrustPostAction::LoadPackage(context) => self.start_package_load(context),
        }
    }

    fn handle_trust_denied(&mut self, post_action: TrustPostAction, message: String) {
        match post_action {
            TrustPostAction::EnterWorkbench => {
                self.startup = WorkbenchStartupState::Workbench;
                self.status = message;
            }
            TrustPostAction::LoadPackage(context) => {
                let report = trust_denied_load_report(context.package_root.clone(), message);
                self.status = "Project trust denied; package load skipped".to_string();
                self.finish_package_load(report);
            }
        }
    }

    fn start_package_load(&mut self, context: CliContext) {
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

    fn start_create_package(
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

    fn drain_startup_task(&mut self) {
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

    fn finish_package_load(&mut self, report: PackageLoadReport) {
        self.status = package_load_status(&report);
        self.package_load_report = Some(report);
        self.startup = WorkbenchStartupState::Workbench;
    }

    fn apply_create_package_report(
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

    fn switch_workbench_root(&mut self, root: &Path) -> Result<(), String> {
        self.explorer =
            Explorer::new(root).map_err(|error| format!("Could not open project root: {error}"))?;
        self.editor = EditorBuffer::new_empty();
        self.input = CommandInput::default();
        self.focus = FocusPane::Explorer;
        self.active_tab = WorkbenchTab::Code;
        self.standard_editor_editing = false;
        self.vim_state = VimState::Normal;
        self.package_load_report = None;
        self.invalidate_workbench_views();
        Ok(())
    }

    fn run(&mut self, terminal: &mut DefaultTerminal) -> io::Result<WorkbenchExit> {
        self.mode = AppMode::Workbench;
        self.exit = None;
        loop {
            self.drain_startup_task();
            self.drain_bytecode_loader();
            terminal.draw(|frame| self.render(frame))?;
            if let Some(exit) = self.exit {
                return Ok(exit);
            }
            if !event::poll(Duration::from_millis(250))? {
                continue;
            }
            match event::read()? {
                Event::Key(key) => {
                    if key.kind == KeyEventKind::Press {
                        self.handle_key_event(key);
                    }
                }
                Event::Mouse(mouse) => self.handle_mouse_event(mouse),
                _ => {}
            }
        }
    }

    pub fn handle_key_event(&mut self, key: KeyEvent) {
        if !self.startup.is_workbench() {
            self.handle_startup_key(key);
            return;
        }

        match self.navigation.translate(key, self.focus) {
            NavigationIntent::Command(command) => self.apply_navigation_command(command),
            NavigationIntent::PassThrough => match self.focus {
                FocusPane::Explorer => self.handle_explorer_key(key),
                FocusPane::Tabs => self.handle_tabs_key(key),
                FocusPane::Editor => self.handle_editor_key(key),
                FocusPane::Input => self.handle_input_key(key),
                FocusPane::Inspector => self.handle_inspector_key(key),
            },
        }
    }

    fn handle_startup_key(&mut self, key: KeyEvent) {
        if is_quit_key(key) {
            self.exit = Some(WorkbenchExit::Quit);
            return;
        }

        let plain = key.modifiers == KeyModifiers::NONE;
        let state = std::mem::replace(&mut self.startup, WorkbenchStartupState::Workbench);

        match state {
            WorkbenchStartupState::Workbench => {
                self.startup = WorkbenchStartupState::Workbench;
            }
            WorkbenchStartupState::InvalidPackageChoice(mut prompt) => match key.code {
                KeyCode::Up | KeyCode::Down | KeyCode::Char('j') | KeyCode::Char('k') if plain => {
                    prompt.selected = prompt.selected.toggle();
                    self.startup = WorkbenchStartupState::InvalidPackageChoice(prompt);
                }
                KeyCode::Char('1') if plain => {
                    prompt.selected = InvalidPackageAction::CreatePackage;
                    self.open_package_name_prompt(prompt);
                }
                KeyCode::Char('2') if plain => {
                    prompt.selected = InvalidPackageAction::ProceedAnyway;
                    self.apply_trust_resolution(
                        prompt.trust_resolution,
                        TrustPostAction::EnterWorkbench,
                    );
                }
                KeyCode::Enter => match prompt.selected {
                    InvalidPackageAction::CreatePackage => self.open_package_name_prompt(prompt),
                    InvalidPackageAction::ProceedAnyway => self.apply_trust_resolution(
                        prompt.trust_resolution,
                        TrustPostAction::EnterWorkbench,
                    ),
                },
                _ => {
                    self.startup = WorkbenchStartupState::InvalidPackageChoice(prompt);
                }
            },
            WorkbenchStartupState::PackageNameEntry(mut prompt) => match key.code {
                KeyCode::Esc if plain => {
                    self.status =
                        "Selected directory is not a valid Move package; choose how to continue"
                            .to_string();
                    self.startup =
                        WorkbenchStartupState::InvalidPackageChoice(InvalidPackagePrompt {
                            root: prompt.parent,
                            message: prompt.invalid_message,
                            trust_resolution: prompt.trust_resolution,
                            selected: InvalidPackageAction::CreatePackage,
                        });
                }
                KeyCode::Enter => {
                    let package_name = prompt.input.text.trim().to_string();
                    if let Some(error) = package_name_error(&package_name) {
                        prompt.error = Some(error.clone());
                        self.status = error;
                        self.startup = WorkbenchStartupState::PackageNameEntry(prompt);
                    } else {
                        self.start_create_package(
                            prompt.parent,
                            package_name,
                            prompt.trust_resolution,
                            prompt.invalid_message,
                        );
                    }
                }
                _ => {
                    prompt.input.handle_key(key);
                    prompt.error = None;
                    self.startup = WorkbenchStartupState::PackageNameEntry(prompt);
                }
            },
            WorkbenchStartupState::TrustDecision(mut prompt) => match key.code {
                KeyCode::Up | KeyCode::Down | KeyCode::Char('j') | KeyCode::Char('k') if plain => {
                    prompt.selected = prompt.selected.toggle();
                    self.startup = WorkbenchStartupState::TrustDecision(prompt);
                }
                KeyCode::Char('1') if plain => {
                    prompt.selected = TrustAction::Trust;
                    self.accept_trust_prompt(prompt);
                }
                KeyCode::Char('2') if plain => {
                    prompt.selected = TrustAction::ContinueWithoutTrust;
                    self.handle_trust_denied(
                        prompt.post_action,
                        "Project trust denied; build, tests, and scanners were skipped."
                            .to_string(),
                    );
                }
                KeyCode::Enter => match prompt.selected {
                    TrustAction::Trust => self.accept_trust_prompt(prompt),
                    TrustAction::ContinueWithoutTrust => self.handle_trust_denied(
                        prompt.post_action,
                        "Project trust denied; build, tests, and scanners were skipped."
                            .to_string(),
                    ),
                },
                _ => {
                    self.startup = WorkbenchStartupState::TrustDecision(prompt);
                }
            },
            WorkbenchStartupState::PackageLoadRunning(state) => {
                self.startup = WorkbenchStartupState::PackageLoadRunning(state);
            }
        }
    }

    fn open_package_name_prompt(&mut self, prompt: InvalidPackagePrompt) {
        let package_name = default_package_name(&prompt.root);
        self.status = "Enter a Move package name".to_string();
        self.startup = WorkbenchStartupState::PackageNameEntry(PackageNamePrompt {
            parent: prompt.root,
            input: CommandInput::from_text(package_name),
            error: None,
            trust_resolution: prompt.trust_resolution,
            invalid_message: prompt.message,
        });
    }

    fn accept_trust_prompt(&mut self, mut prompt: TrustPrompt) {
        match persist_trust_for_resolution(&prompt.resolution) {
            Ok(()) => {
                self.status = format!("Trusted {}", prompt.resolution.trust_target.display());
                self.run_post_trust_action(prompt.post_action);
            }
            Err(error) => {
                prompt.error = Some(error.clone());
                self.status = error;
                self.startup = WorkbenchStartupState::TrustDecision(prompt);
            }
        }
    }

    fn handle_mouse_event(&mut self, mouse: MouseEvent) {
        if !self.startup.is_workbench() {
            return;
        }

        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                self.handle_left_click(mouse.column, mouse.row)
            }
            MouseEventKind::ScrollUp => {
                self.handle_scroll(mouse.column, mouse.row, ScrollDirection::Up)
            }
            MouseEventKind::ScrollDown => {
                self.handle_scroll(mouse.column, mouse.row, ScrollDirection::Down)
            }
            MouseEventKind::ScrollLeft => {
                self.handle_scroll(mouse.column, mouse.row, ScrollDirection::Left)
            }
            MouseEventKind::ScrollRight => {
                self.handle_scroll(mouse.column, mouse.row, ScrollDirection::Right)
            }
            _ => {}
        }
    }

    fn handle_scroll(&mut self, x: u16, y: u16, direction: ScrollDirection) {
        if rect_contains(self.layout.explorer, x, y) {
            self.set_focus(FocusPane::Explorer);
            self.scroll_explorer(direction);
            return;
        }

        if rect_contains(self.layout.editor, x, y) {
            self.set_focus(FocusPane::Editor);
            match self.active_tab {
                WorkbenchTab::Code => self.scroll_editor(direction),
                WorkbenchTab::Bytecode => self.scroll_bytecode(direction),
                WorkbenchTab::Cfg | WorkbenchTab::CallGraph | WorkbenchTab::TypeGraph => {
                    self.scroll_graph(direction)
                }
            }
            return;
        }

        if rect_contains(self.layout.input, x, y) {
            self.set_focus(FocusPane::Input);
            match direction {
                ScrollDirection::Left => self
                    .input
                    .scroll_horizontal(false, MOUSE_HORIZONTAL_SCROLL_STEP),
                ScrollDirection::Right => self
                    .input
                    .scroll_horizontal(true, MOUSE_HORIZONTAL_SCROLL_STEP),
                ScrollDirection::Up | ScrollDirection::Down => {}
            }
            return;
        }

        if self
            .layout
            .inspector
            .is_some_and(|area| rect_contains(area, x, y))
        {
            self.set_focus(FocusPane::Inspector);
        }
    }

    fn scroll_explorer(&mut self, direction: ScrollDirection) {
        match direction {
            ScrollDirection::Up => {
                for _ in 0..MOUSE_VERTICAL_SCROLL_STEP {
                    self.explorer.select_previous();
                }
            }
            ScrollDirection::Down => {
                for _ in 0..MOUSE_VERTICAL_SCROLL_STEP {
                    self.explorer.select_next();
                }
            }
            ScrollDirection::Left | ScrollDirection::Right => {}
        }
    }

    fn scroll_editor(&mut self, direction: ScrollDirection) {
        match direction {
            ScrollDirection::Up => self
                .editor
                .scroll_vertical(false, MOUSE_VERTICAL_SCROLL_STEP),
            ScrollDirection::Down => self
                .editor
                .scroll_vertical(true, MOUSE_VERTICAL_SCROLL_STEP),
            ScrollDirection::Left => self
                .editor
                .scroll_horizontal(false, MOUSE_HORIZONTAL_SCROLL_STEP),
            ScrollDirection::Right => self
                .editor
                .scroll_horizontal(true, MOUSE_HORIZONTAL_SCROLL_STEP),
        }
    }

    fn scroll_bytecode(&mut self, direction: ScrollDirection) {
        match &mut self.bytecode {
            BytecodePane::Selecting(selector) => match direction {
                ScrollDirection::Up => {
                    for _ in 0..MOUSE_VERTICAL_SCROLL_STEP {
                        selector.select_previous();
                    }
                }
                ScrollDirection::Down => {
                    for _ in 0..MOUSE_VERTICAL_SCROLL_STEP {
                        selector.select_next();
                    }
                }
                ScrollDirection::Left | ScrollDirection::Right => {}
            },
            BytecodePane::Ready(session) => match direction {
                ScrollDirection::Up => {
                    session.scroll_vertical(false, MOUSE_VERTICAL_SCROLL_STEP as u16)
                }
                ScrollDirection::Down => {
                    session.scroll_vertical(true, MOUSE_VERTICAL_SCROLL_STEP as u16)
                }
                ScrollDirection::Left => {
                    session.scroll_horizontal(false, MOUSE_HORIZONTAL_SCROLL_STEP as u16)
                }
                ScrollDirection::Right => {
                    session.scroll_horizontal(true, MOUSE_HORIZONTAL_SCROLL_STEP as u16)
                }
            },
            BytecodePane::Empty | BytecodePane::Loading(_) | BytecodePane::Message(_) => {}
        }
    }

    fn scroll_graph(&mut self, direction: ScrollDirection) {
        let Some(GraphPane::Ready(document)) = self.graphs.get_mut(self.active_tab) else {
            return;
        };

        match direction {
            ScrollDirection::Up => document.scroll_vertical(false, MOUSE_VERTICAL_SCROLL_STEP),
            ScrollDirection::Down => document.scroll_vertical(true, MOUSE_VERTICAL_SCROLL_STEP),
            ScrollDirection::Left => {
                document.scroll_horizontal(false, MOUSE_HORIZONTAL_SCROLL_STEP)
            }
            ScrollDirection::Right => {
                document.scroll_horizontal(true, MOUSE_HORIZONTAL_SCROLL_STEP)
            }
        }
    }

    fn handle_left_click(&mut self, x: u16, y: u16) {
        if let Some(tab) = self.clicked_tab(x, y) {
            self.set_active_tab(tab);
            self.set_focus(FocusPane::Editor);
            return;
        }

        if rect_contains(self.layout.tabs, x, y) {
            self.set_focus(FocusPane::Tabs);
            return;
        }

        if rect_contains(self.layout.explorer, x, y) {
            self.handle_explorer_click(x, y);
            return;
        }

        if rect_contains(self.layout.editor, x, y) {
            self.handle_editor_click(x, y);
            return;
        }

        if rect_contains(self.layout.input, x, y) {
            self.handle_input_click(x);
            return;
        }

        if self
            .layout
            .inspector
            .is_some_and(|area| rect_contains(area, x, y))
        {
            self.set_focus(FocusPane::Inspector);
        }
    }

    fn clicked_tab(&self, x: u16, y: u16) -> Option<WorkbenchTab> {
        self.layout
            .tab_hit_areas
            .iter()
            .find_map(|(tab, area)| rect_contains(*area, x, y).then_some(*tab))
    }

    fn handle_explorer_click(&mut self, x: u16, y: u16) {
        self.set_focus(FocusPane::Explorer);
        let inner = inner_rect(self.layout.explorer);
        if !rect_contains(inner, x, y) {
            return;
        }

        let row = usize::from(y.saturating_sub(inner.y));
        if row >= self.explorer.visible_entries().len() {
            return;
        }

        self.explorer.selected = row;
        match self.explorer.activate_selected() {
            ExplorerAction::OpenFile(path) => self.open_file(path),
            ExplorerAction::ToggledDirectory => {
                self.status = String::from("Directory tree updated");
            }
            ExplorerAction::None => {}
        }
    }

    fn handle_editor_click(&mut self, x: u16, y: u16) {
        self.set_focus(FocusPane::Editor);
        if self.active_tab == WorkbenchTab::Code {
            if self.markdown_preview_enabled() {
                return;
            }
            let text_area = self.editor_text_area();
            if rect_contains(text_area, x, y) {
                self.editor.set_cursor_from_view_position(
                    usize::from(y.saturating_sub(text_area.y)),
                    usize::from(x.saturating_sub(text_area.x)),
                );
            }
        }
    }

    fn handle_input_click(&mut self, x: u16) {
        self.set_focus(FocusPane::Input);
        let inner = inner_rect(self.layout.input);
        if x >= inner.x {
            self.input
                .set_cursor_column(usize::from(x.saturating_sub(inner.x)));
        }
    }

    fn apply_navigation_command(&mut self, command: NavigationCommand) {
        match command {
            NavigationCommand::Quit => self.exit = Some(WorkbenchExit::Quit),
            NavigationCommand::Save => self.save_current_file(),
            NavigationCommand::Reload => self.reload_current_file(),
            NavigationCommand::Undo => {
                self.editor.undo();
                self.status = String::from("Undo");
            }
            NavigationCommand::BeginWorkbenchNavigation => {
                self.status = keybinds::workbench_hint();
            }
            NavigationCommand::CancelWorkbenchNavigation => {
                self.status = String::from(navigation::WORKBENCH_CANCELED);
            }
            NavigationCommand::UnboundWorkbenchNavigation => {
                self.status = String::from(navigation::WORKBENCH_UNBOUND);
            }
            NavigationCommand::ToggleEditorMode => self.toggle_editor_mode(),
            NavigationCommand::PreviousTheme => self.previous_theme(),
            NavigationCommand::NextTheme => self.next_theme(),
            NavigationCommand::Focus(pane) => self.set_focus(pane),
            NavigationCommand::FocusCodeEditor => self.focus_code_editor(),
            NavigationCommand::FocusNext => self.set_focus(self.next_focus_pane()),
            NavigationCommand::FocusPrevious => {
                self.set_focus(self.previous_focus_pane());
            }
            NavigationCommand::MoveFocus(direction) => {
                self.set_focus(navigation::move_focus(self.focus, direction));
            }
            NavigationCommand::SelectTab(tab) => {
                self.set_active_tab(tab);
            }
        }
    }

    fn handle_explorer_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Up => self.explorer.select_previous(),
            KeyCode::Down => self.explorer.select_next(),
            KeyCode::Right => match self.explorer.activate_selected() {
                ExplorerAction::OpenFile(path) => self.open_file(path),
                ExplorerAction::ToggledDirectory => {
                    self.status = String::from("Directory tree updated");
                }
                ExplorerAction::None => {}
            },
            KeyCode::Enter => match self.explorer.activate_selected() {
                ExplorerAction::OpenFile(path) => self.open_file(path),
                ExplorerAction::ToggledDirectory => {
                    self.status = String::from("Directory tree updated");
                }
                ExplorerAction::None => {}
            },
            _ => {}
        }
    }

    fn handle_tabs_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Left | KeyCode::Char('h') => self.previous_tab(),
            KeyCode::Right | KeyCode::Char('l') => self.next_tab(),
            KeyCode::Down | KeyCode::Enter | KeyCode::Esc => self.set_focus(FocusPane::Editor),
            _ => {}
        }
    }

    fn handle_editor_key(&mut self, key: KeyEvent) {
        match self.active_tab {
            WorkbenchTab::Code => {
                match self.editor_mode {
                    EditorMode::Standard => self.handle_standard_editor_key(key),
                    EditorMode::Vim => {
                        if self.vim_state == VimState::Insert {
                            if key.code == KeyCode::Esc {
                                self.vim_state = VimState::Normal;
                                self.invalidate_editor_render();
                            } else {
                                self.editor.handle_standard_key(key);
                            }
                        } else {
                            self.handle_vim_normal_key(key);
                        }
                    }
                }
                if self.editor.dirty {
                    self.invalidate_workbench_views();
                }
            }
            WorkbenchTab::Bytecode => self.handle_bytecode_key(key),
            WorkbenchTab::Cfg | WorkbenchTab::CallGraph | WorkbenchTab::TypeGraph => {
                self.handle_graph_key(key)
            }
        }
    }

    fn handle_standard_editor_key(&mut self, key: KeyEvent) {
        if self.standard_editor_editing {
            if key.code == KeyCode::Esc {
                self.standard_editor_editing = false;
                self.invalidate_editor_render();
                self.status = String::from("Editor navigation: Enter or i to edit");
            } else {
                self.editor.handle_standard_key(key);
            }
            return;
        }

        let plain = key.modifiers == KeyModifiers::NONE;
        match key.code {
            KeyCode::Enter | KeyCode::Char('i') if plain => self.enter_standard_editor_editing(),
            KeyCode::Left | KeyCode::Char('h') if plain => self.set_focus(navigation::move_focus(
                self.focus,
                navigation::FocusDirection::Left,
            )),
            KeyCode::Down | KeyCode::Char('j') if plain => self.set_focus(navigation::move_focus(
                self.focus,
                navigation::FocusDirection::Down,
            )),
            KeyCode::Up | KeyCode::Char('k') if plain => self.set_focus(navigation::move_focus(
                self.focus,
                navigation::FocusDirection::Up,
            )),
            KeyCode::Right | KeyCode::Char('l') if plain => self.set_focus(navigation::move_focus(
                self.focus,
                navigation::FocusDirection::Right,
            )),
            KeyCode::Char('e') if plain => self.set_focus(FocusPane::Explorer),
            KeyCode::Char('t') if plain => self.set_focus(FocusPane::Tabs),
            KeyCode::Char('c') if plain => self.focus_code_editor(),
            KeyCode::Char('p') if plain => self.set_focus(FocusPane::Inspector),
            KeyCode::Char('1') if plain => self.set_active_tab(WorkbenchTab::Code),
            KeyCode::Char('2') if plain => self.set_active_tab(WorkbenchTab::Bytecode),
            KeyCode::Char('3') if plain => self.set_active_tab(WorkbenchTab::Cfg),
            KeyCode::Char('4') if plain => self.set_active_tab(WorkbenchTab::CallGraph),
            KeyCode::Char('5') if plain => self.set_active_tab(WorkbenchTab::TypeGraph),
            KeyCode::PageUp if plain => self.editor.page_up(),
            KeyCode::PageDown if plain => self.editor.page_down(),
            KeyCode::Home if plain => self.editor.move_line_start(),
            KeyCode::End if plain => self.editor.move_line_end(),
            _ => {}
        }
    }

    fn enter_standard_editor_editing(&mut self) {
        self.standard_editor_editing = true;
        self.invalidate_editor_render();
        self.status = String::from("Editing: Esc returns to navigation");
    }

    fn handle_bytecode_key(&mut self, key: KeyEvent) {
        if matches!(key.code, KeyCode::Esc | KeyCode::Char('q')) {
            self.active_tab = WorkbenchTab::Code;
            self.standard_editor_editing = false;
            self.set_focus(FocusPane::Editor);
            self.status = String::from("Closed bytecode viewer");
            return;
        }

        let mut request = None;
        let mut show_selector = false;
        let mut load_bytecode = false;

        match &mut self.bytecode {
            BytecodePane::Selecting(selector) => {
                if key.code == KeyCode::Enter {
                    request = selector.selected_request();
                } else {
                    selector.handle_key(key);
                }
            }
            BytecodePane::Ready(session) => {
                if key.code == KeyCode::Enter {
                    show_selector = true;
                } else {
                    session.handle_key(key);
                }
            }
            BytecodePane::Empty | BytecodePane::Message(_) => {
                if key.code == KeyCode::Enter {
                    load_bytecode = true;
                }
            }
            BytecodePane::Loading(_) => {
                if key.code == KeyCode::Enter {
                    self.status = String::from("Bytecode is already loading");
                }
            }
        }

        if load_bytecode {
            self.ensure_bytecode_session();
        }

        if show_selector {
            self.show_bytecode_selector();
        }

        if let Some(request) = request {
            self.load_bytecode_request(request);
        }
    }

    fn handle_graph_key(&mut self, key: KeyEvent) {
        if matches!(key.code, KeyCode::Esc | KeyCode::Char('q')) {
            let title = self.active_tab.title();
            self.active_tab = WorkbenchTab::Code;
            self.standard_editor_editing = false;
            self.set_focus(FocusPane::Editor);
            self.status = format!("Closed {title} viewer");
            return;
        }

        if key.code == KeyCode::Enter {
            self.ensure_graph_tab(self.active_tab);
            return;
        }

        let Some(GraphPane::Ready(document)) = self.graphs.get_mut(self.active_tab) else {
            return;
        };
        document.handle_key(key);
    }

    fn handle_vim_normal_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => self.vim_state = VimState::Normal,
            KeyCode::Char('h') => self.editor.move_left(),
            KeyCode::Char('j') => self.editor.move_down(),
            KeyCode::Char('k') => self.editor.move_up(),
            KeyCode::Char('l') => self.editor.move_right(),
            KeyCode::Char('i') => {
                self.vim_state = VimState::Insert;
                self.invalidate_editor_render();
            }
            KeyCode::Char('a') => {
                self.editor.move_right();
                self.vim_state = VimState::Insert;
                self.invalidate_editor_render();
            }
            KeyCode::Char('A') => {
                self.editor.move_line_end();
                self.vim_state = VimState::Insert;
                self.invalidate_editor_render();
            }
            KeyCode::Char('I') => {
                self.editor.move_line_start();
                self.vim_state = VimState::Insert;
                self.invalidate_editor_render();
            }
            KeyCode::Char('o') => {
                self.editor.open_line_below();
                self.vim_state = VimState::Insert;
                self.invalidate_editor_render();
            }
            KeyCode::Char('O') => {
                self.editor.open_line_above();
                self.vim_state = VimState::Insert;
                self.invalidate_editor_render();
            }
            KeyCode::Char('x') => self.editor.delete_char(),
            KeyCode::Char('u') => self.editor.undo(),
            KeyCode::Char('p') => self.editor.paste_after(),
            KeyCode::Char('d') => {
                if self.editor.pending_vim == Some(PendingVimCommand::Delete) {
                    self.editor.delete_current_line();
                    self.editor.pending_vim = None;
                } else {
                    self.editor.pending_vim = Some(PendingVimCommand::Delete);
                }
            }
            KeyCode::Char('y') => {
                if self.editor.pending_vim == Some(PendingVimCommand::Yank) {
                    self.editor.yank_current_line();
                    self.editor.pending_vim = None;
                } else {
                    self.editor.pending_vim = Some(PendingVimCommand::Yank);
                }
            }
            _ => {
                self.editor.pending_vim = None;
            }
        }
    }

    fn handle_input_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => self.set_focus(FocusPane::Editor),
            KeyCode::Enter => self.dispatch_command_input(),
            _ => self.input.handle_key(key),
        }
    }

    fn dispatch_command_input(&mut self) {
        let text = self.input.take_text();
        match text.trim() {
            "" => {}
            ":agent" | "agent" => {
                self.mode = AppMode::Agent;
                self.status = String::from("Switching to agent mode");
                self.exit = Some(WorkbenchExit::SwitchToAgent);
            }
            command => {
                self.status = format!("Unknown command: {command}");
            }
        }
    }

    fn handle_inspector_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Left | KeyCode::Char('h') | KeyCode::Esc => self.set_focus(FocusPane::Editor),
            _ => {}
        }
    }

    fn focus_code_editor(&mut self) {
        self.standard_editor_editing = false;
        self.active_tab = WorkbenchTab::Code;
        self.set_focus(FocusPane::Editor);
    }

    fn set_active_tab(&mut self, tab: WorkbenchTab) {
        self.active_tab = tab;
        if tab != WorkbenchTab::Code {
            self.standard_editor_editing = false;
        }
        self.set_focus(self.focus);
    }

    fn set_focus(&mut self, pane: FocusPane) {
        let focus = if pane == FocusPane::Inspector && !self.inspector_visible() {
            FocusPane::Editor
        } else {
            pane
        };
        if focus != FocusPane::Editor || self.active_tab != WorkbenchTab::Code {
            self.standard_editor_editing = false;
        }
        self.focus = focus;
    }

    fn next_focus_pane(&self) -> FocusPane {
        if self.inspector_visible() {
            return navigation::next_focus(self.focus);
        }

        let order = hidden_inspector_focus_order();
        let index = order
            .iter()
            .position(|pane| *pane == self.focus)
            .unwrap_or_default();
        order[(index + 1) % order.len()]
    }

    fn previous_focus_pane(&self) -> FocusPane {
        if self.inspector_visible() {
            return navigation::previous_focus(self.focus);
        }

        let order = hidden_inspector_focus_order();
        let index = order
            .iter()
            .position(|pane| *pane == self.focus)
            .unwrap_or_default();
        order[(index + order.len() - 1) % order.len()]
    }

    fn next_tab(&mut self) {
        let index = self.active_tab.index();
        self.set_active_tab(WorkbenchTab::ALL[(index + 1) % WorkbenchTab::ALL.len()]);
    }

    fn previous_tab(&mut self) {
        let index = self.active_tab.index();
        self.set_active_tab(
            WorkbenchTab::ALL[(index + WorkbenchTab::ALL.len() - 1) % WorkbenchTab::ALL.len()],
        );
    }

    fn toggle_editor_mode(&mut self) {
        self.editor_mode = match self.editor_mode {
            EditorMode::Standard => EditorMode::Vim,
            EditorMode::Vim => EditorMode::Standard,
        };
        self.standard_editor_editing = false;
        self.vim_state = VimState::Normal;
        self.invalidate_editor_render();
        self.status = format!("Editor mode: {}", self.editor_mode_label());
    }

    fn previous_theme(&mut self) {
        self.theme.prev();
        self.sync_syntax_theme();
        self.invalidate_editor_render();
        self.status = format!("Theme: {}", self.theme);
    }

    fn next_theme(&mut self) {
        self.theme.next();
        self.sync_syntax_theme();
        self.invalidate_editor_render();
        self.status = format!("Theme: {}", self.theme);
    }

    fn sync_syntax_theme(&self) {
        if let Some(theme) = crate::agent::resolve_theme_by_name(self.theme.name.slug(), None) {
            crate::agent::set_syntax_theme(theme);
        }
    }

    fn editor_mode_label(&self) -> &'static str {
        match self.editor_mode {
            EditorMode::Standard => {
                if self.standard_editor_editing {
                    "standard edit"
                } else {
                    "standard view"
                }
            }
            EditorMode::Vim => match self.vim_state {
                VimState::Normal => "vim normal",
                VimState::Insert => "vim insert",
            },
        }
    }

    fn open_file(&mut self, path: PathBuf) {
        if self.editor.dirty && self.editor.path.as_ref() != Some(&path) {
            self.status = String::from("Unsaved changes: Ctrl-S to save or Ctrl-R to reload first");
            return;
        }

        match self.editor.open_file(&path) {
            Ok(()) => {
                self.invalidate_workbench_views();
                self.active_tab = WorkbenchTab::Code;
                self.standard_editor_editing = false;
                self.set_focus(FocusPane::Editor);
                self.status = format!("Opened {}", path.display());
            }
            Err(error) => {
                self.status = format!("Could not open {}: {error}", path.display());
            }
        }
    }

    fn save_current_file(&mut self) {
        match self.editor.save() {
            Ok(()) => {
                self.invalidate_workbench_views();
                self.status = String::from("Saved");
            }
            Err(error) => self.status = format!("Save failed: {error}"),
        }
    }

    fn reload_current_file(&mut self) {
        match self.editor.reload() {
            Ok(()) => {
                self.invalidate_workbench_views();
                self.status = String::from("Reloaded");
            }
            Err(error) => self.status = format!("Reload failed: {error}"),
        }
    }

    fn invalidate_workbench_views(&mut self) {
        self.invalidate_editor_render();
        self.invalidate_bytecode();
        self.invalidate_graphs();
    }

    fn invalidate_editor_render(&mut self) {
        self.editor_render_cache = None;
    }

    fn invalidate_bytecode(&mut self) {
        self.bytecode.invalidate();
        self.bytecode_cache.clear();
        self.bytecode_loader_rx = None;
        self.bytecode_load_epoch = self.bytecode_load_epoch.wrapping_add(1);
    }

    fn invalidate_graphs(&mut self) {
        self.graphs.invalidate();
    }

    fn palette(&self) -> ThemePalette {
        self.theme.palette()
    }

    fn style_fg(&self, color: Color) -> Style {
        Style::default().fg(color).bg(self.palette().bg)
    }

    fn base_style(&self) -> Style {
        let palette = self.palette();
        Style::default().fg(palette.fg).bg(palette.bg)
    }

    fn muted_style(&self) -> Style {
        self.style_fg(self.palette().muted)
    }

    fn border_style(&self, focused: bool) -> Style {
        let palette = self.palette();
        self.style_fg(if focused {
            palette.accent
        } else {
            palette.graph.edge
        })
    }

    fn title_style(&self, focused: bool) -> Style {
        let palette = self.palette();
        self.style_fg(if focused { palette.accent } else { palette.fg })
            .add_modifier(if focused {
                Modifier::BOLD
            } else {
                Modifier::empty()
            })
    }

    fn selection_style(&self) -> Style {
        let palette = self.palette();
        Style::default()
            .fg(palette.fg)
            .bg(palette.selection)
            .add_modifier(Modifier::BOLD)
    }

    fn panel_block(&self, title: impl Into<String>, focused: bool) -> Block<'static> {
        let title = focused_title(&title.into(), focused);
        Block::default()
            .borders(Borders::ALL)
            .title(title)
            .style(self.base_style())
            .border_style(self.border_style(focused))
            .title_style(self.title_style(focused))
    }

    fn inspector_line(
        &self,
        label: &'static str,
        value: impl Into<String>,
        value_style: Style,
    ) -> Line<'static> {
        Line::from(vec![
            Span::styled(format!("{label}: "), self.muted_style()),
            Span::styled(value.into(), value_style),
        ])
    }

    pub fn render(&mut self, frame: &mut Frame<'_>) {
        let area = frame.area();
        frame.buffer_mut().set_style(area, self.base_style());
        if !self.startup.is_workbench() {
            self.render_startup(frame, area);
            return;
        }

        let show_inspector = self.inspector_visible();
        let constraints = if show_inspector {
            vec![
                Constraint::Percentage(25),
                Constraint::Percentage(50),
                Constraint::Percentage(25),
            ]
        } else {
            vec![Constraint::Percentage(25), Constraint::Percentage(75)]
        };

        let columns = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(constraints)
            .split(area);

        self.layout.explorer = columns[0];
        self.layout.inspector = if show_inspector {
            Some(columns[2])
        } else {
            None
        };
        self.render_explorer(frame, columns[0]);
        self.render_center(frame, columns[1]);
        if show_inspector {
            self.render_inspector(frame, columns[2]);
        }
    }

    fn render_startup(&self, frame: &mut Frame<'_>, area: Rect) {
        let panel_area = centered_rect(area, 76, 22);
        match &self.startup {
            WorkbenchStartupState::InvalidPackageChoice(prompt) => {
                self.render_invalid_package_prompt(frame, panel_area, prompt)
            }
            WorkbenchStartupState::PackageNameEntry(prompt) => {
                self.render_package_name_prompt(frame, panel_area, prompt)
            }
            WorkbenchStartupState::TrustDecision(prompt) => {
                self.render_trust_prompt(frame, panel_area, prompt)
            }
            WorkbenchStartupState::PackageLoadRunning(state) => self.render_startup_message(
                frame,
                panel_area,
                "Package Loading",
                vec![
                    Line::from(state.message.clone()),
                    Line::from(""),
                    Line::styled("Working in the background...", self.muted_style()),
                ],
            ),
            WorkbenchStartupState::Workbench => {}
        }
    }

    fn render_invalid_package_prompt(
        &self,
        frame: &mut Frame<'_>,
        area: Rect,
        prompt: &InvalidPackagePrompt,
    ) {
        let lines = vec![
            Line::styled(
                "Selected directory does not appear to contain a valid Move package.",
                self.style_fg(self.palette().warning)
                    .add_modifier(Modifier::BOLD),
            ),
            Line::from(""),
            self.startup_label_line("directory", prompt.root.display().to_string()),
            self.startup_label_line("details", prompt.message.clone()),
            Line::from(""),
            startup_option_line(
                self,
                prompt.selected == InvalidPackageAction::CreatePackage,
                "1",
                "Create a new Move package in the selected directory",
            ),
            startup_option_line(
                self,
                prompt.selected == InvalidPackageAction::ProceedAnyway,
                "2",
                "Proceed anyway using the selected directory",
            ),
            Line::from(""),
            Line::styled("Use Up/Down or j/k, then Enter.", self.muted_style()),
        ];
        self.render_startup_message(frame, area, "Move Package", lines);
    }

    fn render_package_name_prompt(
        &self,
        frame: &mut Frame<'_>,
        area: Rect,
        prompt: &PackageNamePrompt,
    ) {
        let mut lines = vec![
            Line::styled(
                "Create a new Move package",
                self.style_fg(self.palette().accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Line::from(""),
            self.startup_label_line("parent", prompt.parent.display().to_string()),
            Line::from(""),
            self.startup_label_line("package name", prompt.input.text.clone()),
        ];

        if let Some(error) = &prompt.error {
            lines.push(Line::from(""));
            lines.push(Line::styled(
                error.clone(),
                self.style_fg(self.palette().warning),
            ));
        }

        lines.push(Line::from(""));
        lines.push(Line::styled(
            "Enter creates the package. Esc returns to the previous choice.",
            self.muted_style(),
        ));
        self.render_startup_message(frame, area, "Package Name", lines);
    }

    fn render_trust_prompt(&self, frame: &mut Frame<'_>, area: Rect, prompt: &TrustPrompt) {
        let mut lines = vec![
            Line::styled(
                "Project trust is required before package loading can run.",
                self.style_fg(self.palette().warning)
                    .add_modifier(Modifier::BOLD),
            ),
            Line::from(""),
            self.startup_label_line("directory", prompt.resolution.cwd.display().to_string()),
            self.startup_label_line(
                "trust target",
                prompt.resolution.trust_target.display().to_string(),
            ),
            Line::from(""),
            startup_option_line(
                self,
                prompt.selected == TrustAction::Trust,
                "1",
                "Trust this project and continue",
            ),
            startup_option_line(
                self,
                prompt.selected == TrustAction::ContinueWithoutTrust,
                "2",
                "Continue without trusting this project",
            ),
        ];

        if let Some(error) = &prompt.error {
            lines.push(Line::from(""));
            lines.push(Line::styled(
                error.clone(),
                self.style_fg(self.palette().warning),
            ));
        }

        lines.push(Line::from(""));
        lines.push(Line::styled(
            "Use Up/Down or j/k, then Enter.",
            self.muted_style(),
        ));
        self.render_startup_message(frame, area, "Trust Project", lines);
    }

    fn render_startup_message(
        &self,
        frame: &mut Frame<'_>,
        area: Rect,
        title: &'static str,
        lines: Vec<Line<'static>>,
    ) {
        let paragraph = Paragraph::new(lines)
            .style(self.base_style())
            .block(self.panel_block(title, true))
            .wrap(Wrap { trim: false });
        frame.render_widget(paragraph, area);
    }

    fn startup_label_line(&self, label: &'static str, value: String) -> Line<'static> {
        Line::from(vec![
            Span::styled(format!("{label}: "), self.muted_style()),
            Span::styled(value, self.base_style()),
        ])
    }

    fn inspector_visible(&self) -> bool {
        self.active_tab != WorkbenchTab::Bytecode
    }

    fn render_explorer(&self, frame: &mut Frame<'_>, area: Rect) {
        let palette = self.palette();
        let items = self
            .explorer
            .visible_entries()
            .iter()
            .map(|entry| {
                let marker = if entry.is_dir {
                    if entry.expanded { "[-]" } else { "[+]" }
                } else {
                    "   "
                };
                let suffix = if entry.is_dir { "/" } else { "" };
                let label = format!(
                    "{}{} {}{}",
                    "  ".repeat(entry.depth),
                    marker,
                    entry.name,
                    suffix
                );
                let color = if entry.is_dir {
                    palette.accent
                } else {
                    palette.fg
                };
                ListItem::new(label).style(self.style_fg(color))
            })
            .collect::<Vec<_>>();
        let block = self.panel_block("Explorer", self.focus == FocusPane::Explorer);
        let mut state = ListState::default().with_selected(Some(self.explorer.selected()));
        let list = List::new(items)
            .block(block)
            .style(self.base_style())
            .highlight_style(self.selection_style())
            .highlight_symbol("> ");
        frame.render_stateful_widget(list, area, &mut state);
    }

    fn render_center(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let palette = self.palette();
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(3),
                Constraint::Length(3),
            ])
            .split(area);

        self.layout.tabs = rows[0];
        self.layout.editor = rows[1];
        self.layout.input = rows[2];
        self.layout.tab_hit_areas = tab_hit_areas(&WORKBENCH_TAB_LABELS, rows[0])
            .into_iter()
            .zip(WorkbenchTab::ALL)
            .map(|(area, tab)| (tab, area))
            .collect();

        let tabs = TabNav::new(&WORKBENCH_TAB_LABELS, self.active_tab.index())
            .style(self.muted_style())
            .highlight_style(self.style_fg(palette.accent).add_modifier(Modifier::BOLD))
            .border_style(self.border_style(self.focus == FocusPane::Tabs))
            .highlight_bold(true);
        frame.render_widget(tabs, rows[0]);

        match self.active_tab {
            WorkbenchTab::Code => self.render_editor(frame, rows[1]),
            WorkbenchTab::Bytecode => self.render_bytecode(frame, rows[1]),
            WorkbenchTab::Cfg | WorkbenchTab::CallGraph | WorkbenchTab::TypeGraph => {
                self.render_graph(frame, rows[1], self.active_tab)
            }
        }

        self.render_input(frame, rows[2]);
    }

    fn render_graph(&mut self, frame: &mut Frame<'_>, area: Rect, tab: WorkbenchTab) {
        let focused = self.focus == FocusPane::Editor;
        let message_style = self.muted_style();
        let graph_style = self.style_fg(self.palette().syntax.text);
        let block = self.panel_block(tab.title(), focused);
        let inner = inner_rect(area);

        match self.graphs.get_mut(tab) {
            Some(GraphPane::Ready(document)) => {
                document.set_viewport_size(inner.height as usize, inner.width as usize);
                let paragraph = Paragraph::new(document.text.as_str())
                    .style(graph_style)
                    .block(block)
                    .scroll((
                        usize_to_u16_saturating(document.scroll),
                        usize_to_u16_saturating(document.horizontal_scroll),
                    ));
                frame.render_widget(paragraph, area);
            }
            Some(GraphPane::Message(message)) => {
                let paragraph = Paragraph::new(message.as_str())
                    .style(message_style)
                    .block(block);
                frame.render_widget(paragraph, area);
            }
            Some(GraphPane::Empty) | None => {
                let paragraph = Paragraph::new(format!(
                    "{} is not loaded. Press Enter to load.",
                    tab.title()
                ))
                .style(message_style)
                .block(block);
                frame.render_widget(paragraph, area);
            }
        }
    }

    fn render_bytecode(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let palette = self.theme.palette();
        let focused = self.focus == FocusPane::Editor;
        let light_theme = self.theme.is_light();
        let message_style = self.muted_style();
        let message_block = self.panel_block("Bytecode", focused);

        match &mut self.bytecode {
            BytecodePane::Ready(session) => {
                session.render(frame, area, palette, focused, light_theme);
            }
            BytecodePane::Selecting(selector) => {
                selector.render(frame, area, palette, focused);
            }
            BytecodePane::Loading(load) => {
                let paragraph = Paragraph::new(format!(
                    "Loading bytecode for {}::{}...",
                    load.package_name, load.module_name
                ))
                .style(message_style)
                .block(message_block);
                frame.render_widget(paragraph, area);
            }
            BytecodePane::Message(message) => {
                let paragraph = Paragraph::new(message.as_str())
                    .style(message_style)
                    .block(message_block);
                frame.render_widget(paragraph, area);
            }
            BytecodePane::Empty => {
                let paragraph =
                    Paragraph::new("Bytecode is not loaded. Press Enter to resolve modules.")
                        .style(message_style)
                        .block(message_block);
                frame.render_widget(paragraph, area);
            }
        }
    }

    fn ensure_bytecode_session(&mut self) {
        if matches!(&self.bytecode, BytecodePane::Loading(_)) {
            self.status = String::from("Bytecode is already loading");
            return;
        }

        let options = match self.current_bytecode_options() {
            Ok(options) => options,
            Err(message) => {
                self.bytecode.set_message(message);
                return;
            }
        };

        if self.bytecode.ready_matches_any(&options)
            || self.bytecode.selector_matches(&options)
            || self.bytecode.loading_matches_any(&options)
        {
            return;
        }

        match options.targets.as_slice() {
            [] => self
                .bytecode
                .set_message("No Move module matched the requested bytecode target.".to_string()),
            [target] => {
                let request = BytecodeRequest::new(options.context, target.clone());
                self.load_bytecode_request(request);
            }
            _ => {
                self.status = format!("Select a module from {}", options.package_name);
                self.bytecode = BytecodePane::Selecting(BytecodeSelector::new(options));
            }
        }
    }

    fn load_bytecode_request(&mut self, request: BytecodeRequest) {
        let stamp = bytecode_cache_stamp(&request.context.package_root);
        if let Some(entry) = self.bytecode_cache.get(&request.key).cloned() {
            if entry.stamp == stamp {
                self.status = format!(
                    "Loaded bytecode for {}::{} from cache",
                    entry.session.package_name, entry.session.key.module_name
                );
                self.bytecode = BytecodePane::Ready(entry.session);
                return;
            }
            self.bytecode_cache.remove(&request.key);
        }

        if self.bytecode.is_loading_key(&request.key) {
            self.status = String::from("Bytecode is already loading");
            return;
        }

        self.bytecode_load_epoch = self.bytecode_load_epoch.wrapping_add(1);
        let epoch = self.bytecode_load_epoch;
        let key = request.key.clone();
        let result_key = key.clone();
        let package_name = request.package_name.clone();
        let module_name = request.key.module_name.clone();
        let (tx, rx) = mpsc::channel();

        match thread::Builder::new()
            .name(format!("peregrine-bytecode-{module_name}"))
            .spawn(move || {
                let result = BytecodeSession::load(request);
                let _ = tx.send(BytecodeLoadResult {
                    epoch,
                    key: result_key,
                    stamp,
                    result,
                });
            }) {
            Ok(_) => {
                self.bytecode_loader_rx = Some(rx);
                self.status = format!("Loading bytecode for {package_name}::{module_name}");
                self.bytecode = BytecodePane::Loading(BytecodeLoadState {
                    key,
                    package_name,
                    module_name,
                    stamp,
                    epoch,
                });
            }
            Err(error) => {
                let message = format!("Could not start bytecode loader: {error}");
                self.status = format!("Bytecode failed: {message}");
                self.bytecode = BytecodePane::Message(message);
            }
        }
    }

    fn drain_bytecode_loader(&mut self) {
        let event = match self.bytecode_loader_rx.as_ref() {
            Some(rx) => match rx.try_recv() {
                Ok(result) => Some(Ok(result)),
                Err(mpsc::TryRecvError::Empty) => None,
                Err(mpsc::TryRecvError::Disconnected) => Some(Err(
                    "Bytecode loader stopped before returning a result.".to_string(),
                )),
            },
            None => None,
        };

        match event {
            Some(Ok(result)) => {
                self.bytecode_loader_rx = None;
                self.apply_bytecode_load_result(result);
            }
            Some(Err(message)) => {
                self.bytecode_loader_rx = None;
                if matches!(&self.bytecode, BytecodePane::Loading(_)) {
                    self.status = format!("Bytecode failed: {message}");
                    self.bytecode = BytecodePane::Message(message);
                }
            }
            None => {}
        }
    }

    fn apply_bytecode_load_result(&mut self, result: BytecodeLoadResult) {
        let is_current = matches!(
            &self.bytecode,
            BytecodePane::Loading(load)
                if load.epoch == result.epoch
                    && load.key == result.key
                    && load.stamp == result.stamp
        );

        if !is_current {
            return;
        }

        if bytecode_cache_stamp(&result.key.package_root) != result.stamp {
            self.status =
                String::from("Package changed while bytecode was loading; press Enter to reload");
            self.bytecode = BytecodePane::Empty;
            return;
        }

        match result.result {
            Ok(session) => {
                self.status = format!(
                    "Loaded bytecode for {}::{}",
                    session.package_name, session.key.module_name
                );
                self.bytecode_cache.insert(
                    result.key.clone(),
                    BytecodeCacheEntry {
                        stamp: result.stamp,
                        session: session.clone(),
                    },
                );
                self.bytecode = BytecodePane::Ready(session);
            }
            Err(message) => {
                self.status = format!("Bytecode failed: {message}");
                self.bytecode = BytecodePane::Message(message);
            }
        }
    }

    fn show_bytecode_selector(&mut self) {
        match self.current_bytecode_options() {
            Ok(options) if options.targets.is_empty() => self
                .bytecode
                .set_message("No Move module matched the requested bytecode target.".to_string()),
            Ok(options) => {
                self.status = format!("Select a module from {}", options.package_name);
                self.bytecode = BytecodePane::Selecting(BytecodeSelector::new(options));
            }
            Err(message) => self.bytecode.set_message(message),
        }
    }

    fn current_bytecode_options(&self) -> Result<BytecodeOptions, String> {
        if self.editor.dirty {
            return Err("Save the current file before opening the bytecode view.".to_string());
        }

        let project_root = self.explorer.root.clone();
        let source_hint = self
            .editor
            .path
            .as_ref()
            .cloned()
            .or_else(|| self.explorer.selected_path().map(Path::to_path_buf));
        let package_root = source_hint
            .as_deref()
            .and_then(|path| nearest_move_package_root(path, &project_root))
            .or_else(|| nearest_move_package_root(&project_root, &project_root))
            .ok_or_else(|| {
                "Open or select a Move source file inside a package with Move.toml.".to_string()
            })?;
        let package_path = relative_path_label(&project_root, &package_root);
        let context = resolve_context(&project_root, &package_path)
            .map_err(|error| error.message.to_string())?;
        let file = source_hint
            .as_ref()
            .filter(|path| path.is_file() && path.extension() == Some(OsStr::new("move")))
            .and_then(|path| path.strip_prefix(&project_root).ok())
            .map(normalized_path_string);
        let targets =
            bytecode_targets(&context, None, file.as_deref()).map_err(|error| error.message)?;

        Ok(BytecodeOptions::new(context, file, targets))
    }

    fn ensure_graph_tab(&mut self, tab: WorkbenchTab) {
        if matches!(self.graphs.get(tab), Some(GraphPane::Ready(_))) {
            self.status = format!("{} already loaded", tab.title());
            return;
        }

        let result = self.load_graph_document(tab);
        match result {
            Ok(document) => {
                self.status = format!("Loaded {}", document.title);
                self.graphs.set_ready(tab, document);
            }
            Err(message) => {
                self.status = format!("{} failed", tab.title());
                self.graphs.set_message(tab, message);
            }
        }
    }

    fn load_graph_document(&self, tab: WorkbenchTab) -> Result<GraphDocument, String> {
        let context = self.current_graph_context()?;
        match tab {
            WorkbenchTab::Cfg => self.load_cfg_graph_document(&context),
            WorkbenchTab::CallGraph => self.load_call_graph_document(&context),
            WorkbenchTab::TypeGraph => self.load_type_graph_document(&context),
            WorkbenchTab::Code | WorkbenchTab::Bytecode => {
                Err(format!("{} is not a graph tab.", tab.title()))
            }
        }
    }

    fn current_graph_context(&self) -> Result<WorkbenchGraphContext, String> {
        if self.editor.dirty {
            return Err("Save the current file before loading graph views.".to_string());
        }

        let project_root = self.explorer.root.clone();
        let source_hint = self
            .editor
            .path
            .as_ref()
            .cloned()
            .or_else(|| self.explorer.selected_path().map(Path::to_path_buf));
        let package_root = source_hint
            .as_deref()
            .and_then(|path| nearest_move_package_root(path, &project_root))
            .or_else(|| nearest_move_package_root(&project_root, &project_root))
            .ok_or_else(|| {
                "Open or select a Move source file inside a package with Move.toml.".to_string()
            })?;
        let package_path = relative_path_label(&project_root, &package_root);
        let context = resolve_context(&project_root, &package_path)
            .map_err(|error| error.message.to_string())?;
        let file = source_hint
            .as_ref()
            .filter(|path| path.is_file() && path.extension() == Some(OsStr::new("move")))
            .and_then(|path| path.strip_prefix(&project_root).ok())
            .map(normalized_path_string);
        let module_filters = match file.as_deref() {
            Some(file) => bytecode_targets(&context, None, Some(file))
                .map_err(|error| error.message)?
                .into_iter()
                .map(|target| target.module_name)
                .collect(),
            None => Vec::new(),
        };

        Ok(WorkbenchGraphContext {
            context,
            module_filters,
        })
    }

    fn load_cfg_graph_document(
        &self,
        graph_context: &WorkbenchGraphContext,
    ) -> Result<GraphDocument, String> {
        let module = if graph_context.module_filters.len() == 1 {
            graph_context.module_filters.first().cloned()
        } else {
            None
        };
        let args = CfgArgs {
            module,
            function: None,
            output: text_graph_output_args(),
        };

        graph_step_document(WorkbenchTab::Cfg, run_cfg(&graph_context.context, &args))
    }

    fn load_call_graph_document(
        &self,
        graph_context: &WorkbenchGraphContext,
    ) -> Result<GraphDocument, String> {
        let args = CallGraphArgs {
            modules: graph_context.module_filters.clone(),
            include_external: false,
            output: text_graph_output_args(),
        };

        graph_step_document(
            WorkbenchTab::CallGraph,
            run_call_graph(&graph_context.context, &args),
        )
    }

    fn load_type_graph_document(
        &self,
        graph_context: &WorkbenchGraphContext,
    ) -> Result<GraphDocument, String> {
        let graph = discover_move_project_graphs_for_package(
            &graph_context.context.project_root,
            &graph_context.context.package_path,
        )
        .type_graph;
        let graph = filter_type_graph(graph, &graph_context.module_filters);

        if graph.nodes.is_empty() {
            return Err("No type graph nodes matched the requested target.".to_string());
        }

        Ok(GraphDocument::new(
            WorkbenchTab::TypeGraph.title(),
            render_type_graph_text(&graph),
        ))
    }

    fn editor_text_area(&self) -> Rect {
        self.editor_areas_with_gutter(self.layout.editor, !self.markdown_preview_enabled())
            .1
    }

    fn markdown_preview_enabled(&self) -> bool {
        self.editor.path.as_deref().is_some_and(is_markdown_path) && !self.editor_source_editing()
    }

    fn editor_source_editing(&self) -> bool {
        match self.editor_mode {
            EditorMode::Standard => self.standard_editor_editing,
            EditorMode::Vim => self.vim_state == VimState::Insert,
        }
    }

    fn rendered_editor_document(
        &mut self,
        source: &str,
        width: usize,
        markdown_preview: bool,
    ) -> RenderedWorkbenchDocument {
        let path = self.editor.path.clone();
        let root = self.explorer.root.clone();
        let theme = self.theme.name;

        if let Some(cache) = &self.editor_render_cache
            && cache.path == path
            && cache.source == source
            && cache.theme == theme
            && cache.markdown_preview == markdown_preview
            && cache.width == width
            && cache.root == root
        {
            return cache.document.clone();
        }

        let document = render_workbench_document(
            source,
            path.as_deref(),
            self.palette(),
            markdown_preview,
            width,
            Some(&root),
        );
        self.editor_render_cache = Some(EditorRenderCache {
            path,
            source: source.to_string(),
            theme,
            markdown_preview,
            width,
            root,
            document: document.clone(),
        });
        document
    }

    fn editor_areas_with_gutter(&self, area: Rect, show_gutter: bool) -> (Rect, Rect) {
        let inner = inner_rect(area);
        if inner.width == 0 {
            return (inner, inner);
        }

        let desired_gutter_width = usize_to_u16_saturating(self.editor.line_number_gutter_width());
        let gutter_width = if !show_gutter || inner.width <= 1 {
            0
        } else {
            desired_gutter_width.min(inner.width.saturating_sub(1))
        };
        let gutter = Rect {
            x: inner.x,
            y: inner.y,
            width: gutter_width,
            height: inner.height,
        };
        let text = Rect {
            x: inner.x.saturating_add(gutter_width),
            y: inner.y,
            width: inner.width.saturating_sub(gutter_width),
            height: inner.height,
        };

        (gutter, text)
    }

    fn render_editor(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let markdown_preview = self.markdown_preview_enabled();
        let (gutter_area, text_area) = self.editor_areas_with_gutter(area, !markdown_preview);
        let inner_height = text_area.height as usize;
        let inner_width = text_area.width as usize;
        self.editor.set_viewport_size(inner_height, inner_width);
        let title = format!(
            "{}{} [{}]",
            self.editor.display_name(),
            if self.editor.dirty { " *" } else { "" },
            self.editor_mode_label()
        );
        let block = self.panel_block(title, self.focus == FocusPane::Editor);
        frame.render_widget(block, area);

        let text = self.editor.text();
        let rendered = self.rendered_editor_document(&text, inner_width, markdown_preview);

        if rendered.show_gutter && gutter_area.width > 0 && gutter_area.height > 0 {
            let numbers = self.editor.line_numbers_text();
            let gutter = Paragraph::new(numbers)
                .style(self.muted_style())
                .scroll((usize_to_u16_saturating(self.editor.scroll), 0));
            frame.render_widget(gutter, gutter_area);
        }

        if !rendered.show_cursor {
            self.editor.scroll = self
                .editor
                .scroll
                .min(rendered.lines.len().saturating_sub(inner_height));
        }

        let show_cursor = rendered.show_cursor;
        let paragraph = Paragraph::new(rendered.lines)
            .style(self.style_fg(self.palette().syntax.text))
            .scroll((
                usize_to_u16_saturating(self.editor.scroll),
                usize_to_u16_saturating(self.editor.horizontal_scroll),
            ));
        frame.render_widget(paragraph, text_area);

        if show_cursor && self.focus == FocusPane::Editor && self.active_tab == WorkbenchTab::Code {
            let row = self.editor.cursor.row.saturating_sub(self.editor.scroll);
            let col = self
                .editor
                .cursor
                .col
                .saturating_sub(self.editor.horizontal_scroll);
            if self.editor.cursor.row >= self.editor.scroll
                && self.editor.cursor.col >= self.editor.horizontal_scroll
                && row < inner_height
                && col < inner_width
            {
                let x = usize_to_u16_saturating(col);
                let y = usize_to_u16_saturating(row);
                frame.set_cursor_position(Position::new(text_area.x + x, text_area.y + y));
            }
        }
    }

    fn render_input(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let inner_width = area.width.saturating_sub(2) as usize;
        self.input.set_viewport_width(inner_width);
        let title = format!("Input - {}", self.status);
        let paragraph = Paragraph::new(self.input.text.as_str())
            .style(self.base_style())
            .block(self.panel_block(title, self.focus == FocusPane::Input))
            .scroll((0, usize_to_u16_saturating(self.input.scroll)));
        frame.render_widget(paragraph, area);

        if self.focus == FocusPane::Input {
            let col = self.input.cursor.saturating_sub(self.input.scroll);
            if self.input.cursor >= self.input.scroll && col < inner_width {
                let x = usize_to_u16_saturating(col);
                frame.set_cursor_position(Position::new(area.x + 1 + x, area.y + 1));
            }
        }
    }

    fn render_inspector(&self, frame: &mut Frame<'_>, area: Rect) {
        let palette = self.palette();
        let mut lines = Vec::new();
        if let Some(state) = self.package_load_running_state() {
            lines.push(Line::from(vec![
                Span::styled(
                    package_load_spinner(state.started_at),
                    self.style_fg(palette.accent).add_modifier(Modifier::BOLD),
                ),
                Span::raw(" "),
                Span::styled("running", self.style_fg(palette.warning)),
            ]));
            lines.push(self.inspector_line(
                "elapsed",
                format_elapsed(state.started_at.elapsed()),
                self.muted_style(),
            ));
        } else if let Some(report) = &self.package_load_report {
            lines.extend(package_load_status_lines(report, self));
        } else {
            lines.push(Line::styled(
                "Package",
                self.style_fg(palette.info).add_modifier(Modifier::BOLD),
            ));
            lines.push(Line::styled("No package status yet", self.muted_style()));
        }
        let paragraph = Paragraph::new(lines)
            .style(self.base_style())
            .block(self.panel_block("Inspector", self.focus == FocusPane::Inspector))
            .wrap(Wrap { trim: false });
        frame.render_widget(paragraph, area);
    }

    fn package_load_running_state(&self) -> Option<&PackageLoadRunningState> {
        match &self.startup {
            WorkbenchStartupState::PackageLoadRunning(state) => Some(state),
            _ => None,
        }
    }
}

#[derive(Debug, Default)]
enum BytecodePane {
    #[default]
    Empty,
    Selecting(BytecodeSelector),
    Loading(BytecodeLoadState),
    Ready(BytecodeSession),
    Message(String),
}

impl BytecodePane {
    fn invalidate(&mut self) {
        *self = Self::Empty;
    }

    fn set_message(&mut self, message: String) {
        match self {
            Self::Message(current) if current == &message => {}
            _ => *self = Self::Message(message),
        }
    }

    fn ready_matches_any(&self, options: &BytecodeOptions) -> bool {
        matches!(self, Self::Ready(session) if options.contains_target_key(&session.key))
    }

    fn loading_matches_any(&self, options: &BytecodeOptions) -> bool {
        matches!(self, Self::Loading(load) if options.contains_target_key(&load.key))
    }

    fn is_loading_key(&self, key: &BytecodeTargetKey) -> bool {
        matches!(self, Self::Loading(load) if load.key == *key)
    }

    fn selector_matches(&self, options: &BytecodeOptions) -> bool {
        matches!(self, Self::Selecting(selector) if selector.matches(options))
    }
}

#[derive(Debug, Default)]
struct GraphPanes {
    cfg: GraphPane,
    call_graph: GraphPane,
    type_graph: GraphPane,
}

impl GraphPanes {
    fn invalidate(&mut self) {
        *self = Self::default();
    }

    fn get(&self, tab: WorkbenchTab) -> Option<&GraphPane> {
        match tab {
            WorkbenchTab::Cfg => Some(&self.cfg),
            WorkbenchTab::CallGraph => Some(&self.call_graph),
            WorkbenchTab::TypeGraph => Some(&self.type_graph),
            WorkbenchTab::Code | WorkbenchTab::Bytecode => None,
        }
    }

    fn get_mut(&mut self, tab: WorkbenchTab) -> Option<&mut GraphPane> {
        match tab {
            WorkbenchTab::Cfg => Some(&mut self.cfg),
            WorkbenchTab::CallGraph => Some(&mut self.call_graph),
            WorkbenchTab::TypeGraph => Some(&mut self.type_graph),
            WorkbenchTab::Code | WorkbenchTab::Bytecode => None,
        }
    }

    fn set_ready(&mut self, tab: WorkbenchTab, document: GraphDocument) {
        if let Some(pane) = self.get_mut(tab) {
            *pane = GraphPane::Ready(document);
        }
    }

    fn set_message(&mut self, tab: WorkbenchTab, message: String) {
        if let Some(pane) = self.get_mut(tab) {
            *pane = GraphPane::Message(message);
        }
    }
}

#[derive(Debug, Default)]
enum GraphPane {
    #[default]
    Empty,
    Ready(GraphDocument),
    Message(String),
}

#[derive(Debug, Clone)]
struct GraphDocument {
    title: String,
    text: String,
    line_count: usize,
    max_width: usize,
    scroll: usize,
    horizontal_scroll: usize,
    viewport_height: usize,
    viewport_width: usize,
}

impl GraphDocument {
    fn new(title: impl Into<String>, text: impl Into<String>) -> Self {
        let text = text.into();
        let line_count = text.lines().count().max(1);
        let max_width = text.lines().map(char_len).max().unwrap_or_default();

        Self {
            title: title.into(),
            text,
            line_count,
            max_width,
            scroll: 0,
            horizontal_scroll: 0,
            viewport_height: 1,
            viewport_width: 1,
        }
    }

    fn set_viewport_size(&mut self, height: usize, width: usize) {
        self.viewport_height = height.max(1);
        self.viewport_width = width.max(1);
        self.clamp_scrolls();
    }

    fn handle_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => self.scroll_vertical(false, 1),
            KeyCode::Down | KeyCode::Char('j') => self.scroll_vertical(true, 1),
            KeyCode::PageUp => self.scroll_vertical(false, PAGE_SIZE),
            KeyCode::PageDown => self.scroll_vertical(true, PAGE_SIZE),
            KeyCode::Left | KeyCode::Char('h') => self.scroll_horizontal(false, 1),
            KeyCode::Right | KeyCode::Char('l') => self.scroll_horizontal(true, 1),
            KeyCode::Home => {
                self.scroll = 0;
                self.horizontal_scroll = 0;
            }
            KeyCode::End => {
                self.scroll = self.max_vertical_scroll();
                self.horizontal_scroll = self.max_horizontal_scroll();
            }
            _ => {}
        }
    }

    fn scroll_vertical(&mut self, down: bool, amount: usize) {
        if down {
            self.scroll = self
                .scroll
                .saturating_add(amount)
                .min(self.max_vertical_scroll());
        } else {
            self.scroll = self.scroll.saturating_sub(amount);
        }
    }

    fn scroll_horizontal(&mut self, right: bool, amount: usize) {
        if right {
            self.horizontal_scroll = self
                .horizontal_scroll
                .saturating_add(amount)
                .min(self.max_horizontal_scroll());
        } else {
            self.horizontal_scroll = self.horizontal_scroll.saturating_sub(amount);
        }
    }

    fn clamp_scrolls(&mut self) {
        self.scroll = self.scroll.min(self.max_vertical_scroll());
        self.horizontal_scroll = self.horizontal_scroll.min(self.max_horizontal_scroll());
    }

    fn max_vertical_scroll(&self) -> usize {
        self.line_count.saturating_sub(self.viewport_height)
    }

    fn max_horizontal_scroll(&self) -> usize {
        self.max_width
            .saturating_add(1)
            .saturating_sub(self.viewport_width)
    }
}

#[derive(Debug)]
struct WorkbenchGraphContext {
    context: CliContext,
    module_filters: Vec<String>,
}

fn text_graph_output_args() -> GraphOutputArgs {
    GraphOutputArgs {
        dot: false,
        output: None,
    }
}

fn graph_step_document(tab: WorkbenchTab, step: CliStep) -> Result<GraphDocument, String> {
    if step.status != CliStatus::Passed {
        return Err(render_graph_step_error(&step));
    }

    Ok(GraphDocument::new(
        tab.title(),
        strip_ansi_sequences(step.stdout.trim_end()),
    ))
}

fn render_graph_step_error(step: &CliStep) -> String {
    let mut lines = Vec::new();

    for diagnostic in &step.diagnostics {
        lines.push(format!("{}: {}", diagnostic.source, diagnostic.message));
    }

    if !step.stdout.trim().is_empty() {
        lines.push("stdout:".to_string());
        lines.extend(
            strip_ansi_sequences(step.stdout.trim_end())
                .lines()
                .map(|line| format!("  {line}")),
        );
    }

    if !step.stderr.trim().is_empty() {
        lines.push("stderr:".to_string());
        lines.extend(
            strip_ansi_sequences(step.stderr.trim_end())
                .lines()
                .map(|line| format!("  {line}")),
        );
    }

    if lines.is_empty() {
        lines.push(format!("{} failed.", step.name));
    }

    lines.join("\n")
}

fn strip_ansi_sequences(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch != '\x1b' {
            output.push(ch);
            continue;
        }

        match chars.peek().copied() {
            Some('[') => {
                chars.next();
                for code in chars.by_ref() {
                    if ('@'..='~').contains(&code) {
                        break;
                    }
                }
            }
            Some(']') => {
                chars.next();
                let mut escaped = false;
                for code in chars.by_ref() {
                    if escaped && code == '\\' {
                        break;
                    }
                    escaped = code == '\x1b';
                    if code == '\x07' {
                        break;
                    }
                }
            }
            Some(_) | None => {}
        }
    }

    output
}

fn filter_type_graph(graph: MoveTypeGraph, module_filters: &[String]) -> MoveTypeGraph {
    let requested_modules = module_filters
        .iter()
        .map(|module| module.trim())
        .filter(|module| !module.is_empty())
        .collect::<Vec<_>>();
    let selected_ids = graph
        .nodes
        .iter()
        .filter(|node| !node.is_external)
        .filter(|node| {
            requested_modules.is_empty()
                || requested_modules
                    .iter()
                    .any(|requested| type_graph_node_matches(node, requested))
        })
        .map(|node| node.id.clone())
        .collect::<BTreeSet<_>>();

    if selected_ids.is_empty() {
        return MoveTypeGraph {
            nodes: Vec::new(),
            edges: Vec::new(),
            unresolved_types: Vec::new(),
        };
    }

    let mut node_ids = selected_ids.clone();
    let mut edges = Vec::new();
    for edge in graph.edges {
        if selected_ids.contains(&edge.source) || selected_ids.contains(&edge.target) {
            node_ids.insert(edge.source.clone());
            node_ids.insert(edge.target.clone());
            edges.push(edge);
        }
    }

    let unresolved_types = graph
        .unresolved_types
        .into_iter()
        .filter(|unresolved| selected_ids.contains(&unresolved.source))
        .collect::<Vec<_>>();
    let nodes = graph
        .nodes
        .into_iter()
        .filter(|node| node_ids.contains(&node.id))
        .collect::<Vec<_>>();

    MoveTypeGraph {
        nodes,
        edges,
        unresolved_types,
    }
}

fn type_graph_node_matches(node: &MoveTypeGraphNode, requested: &str) -> bool {
    let Some(module_name) = node.module_name.as_deref() else {
        return false;
    };
    let address = node
        .address
        .as_deref()
        .or(node.canonical_address.as_deref());

    graph_module_matches(requested, address, module_name)
}

fn graph_module_matches(requested: &str, address: Option<&str>, module_name: &str) -> bool {
    if requested == module_name {
        return true;
    }

    address.is_some_and(|address| requested == format!("{address}::{module_name}"))
}

fn render_type_graph_text(graph: &MoveTypeGraph) -> String {
    let nodes = graph
        .nodes
        .iter()
        .map(|node| (node.id.as_str(), node))
        .collect::<BTreeMap<_, _>>();
    let mut outgoing = BTreeMap::<&str, Vec<&MoveTypeGraphEdge>>::new();
    let mut unresolved = BTreeMap::<&str, Vec<&MoveUnresolvedType>>::new();

    for edge in &graph.edges {
        outgoing.entry(edge.source.as_str()).or_default().push(edge);
    }
    for ty in &graph.unresolved_types {
        unresolved.entry(ty.source.as_str()).or_default().push(ty);
    }

    let mut modules = BTreeMap::<String, Vec<&MoveTypeGraphNode>>::new();
    for node in &graph.nodes {
        modules
            .entry(type_graph_module_label(node))
            .or_default()
            .push(node);
    }

    let mut lines = vec![format!(
        "type graph nodes={} edges={} unresolved={}",
        graph.nodes.len(),
        graph.edges.len(),
        graph.unresolved_types.len()
    )];

    for (module, mut module_nodes) in modules {
        module_nodes.sort_by(|left, right| {
            left.kind
                .cmp(&right.kind)
                .then_with(|| left.name.cmp(&right.name))
        });
        lines.push(format!("|-- module {module}"));

        for node in module_nodes {
            let external = if node.is_external { " external" } else { "" };
            let abilities = if node.abilities.is_empty() {
                String::new()
            } else {
                format!(" abilities={}", node.abilities.join(","))
            };
            let attributes = if node.attributes.is_empty() {
                String::new()
            } else {
                format!(" attrs={}", node.attributes.join(","))
            };
            lines.push(format!(
                "|   |-- {} {} [{}{}{}{}]",
                node.kind,
                type_graph_node_label(node),
                node.source,
                external,
                abilities,
                attributes
            ));

            for edge in outgoing.get(node.id.as_str()).into_iter().flatten() {
                let target = nodes
                    .get(edge.target.as_str())
                    .map(|target| type_graph_node_qualified_label(target))
                    .unwrap_or_else(|| edge.target.clone());
                lines.push(format!(
                    "|   |   |-- {} -> {}",
                    type_graph_edge_label(edge),
                    target
                ));
            }

            for ty in unresolved.get(node.id.as_str()).into_iter().flatten() {
                lines.push(format!(
                    "|   |   |-- unresolved {} in {}: {}",
                    ty.raw_type, ty.context, ty.reason
                ));
            }
        }
    }

    lines.join("\n")
}

fn type_graph_module_label(node: &MoveTypeGraphNode) -> String {
    match (node.address.as_deref(), node.module_name.as_deref()) {
        (Some(address), Some(module)) => format!("{address}::{module}"),
        (None, Some(module)) => module.to_string(),
        _ if node.is_external => "<external>".to_string(),
        _ => "<unknown>".to_string(),
    }
}

fn type_graph_node_label(node: &MoveTypeGraphNode) -> String {
    if node.type_parameters.is_empty() {
        return node.name.clone();
    }

    let parameters = node
        .type_parameters
        .iter()
        .map(|parameter| {
            let mut label = String::new();
            if parameter.is_phantom {
                label.push_str("phantom ");
            }
            label.push_str(&parameter.name);
            if !parameter.abilities.is_empty() {
                label.push_str(": ");
                label.push_str(&parameter.abilities.join("+"));
            }
            label
        })
        .collect::<Vec<_>>()
        .join(", ");

    format!("{}<{parameters}>", node.name)
}

fn type_graph_node_qualified_label(node: &MoveTypeGraphNode) -> String {
    if node.qualified_name.is_empty() {
        type_graph_node_label(node)
    } else {
        node.qualified_name.clone()
    }
}

fn type_graph_edge_label(edge: &MoveTypeGraphEdge) -> String {
    let mut details = Vec::new();

    if let Some(field_name) = &edge.field_name {
        details.push(format!("field={field_name}"));
    }
    if let Some(variant_name) = &edge.variant_name {
        details.push(format!("variant={variant_name}"));
    }
    if let Some(function_name) = &edge.function_name {
        details.push(format!("function={function_name}"));
    }
    if let Some(parameter_name) = &edge.parameter_name {
        details.push(format!("param={parameter_name}"));
    }
    if let Some(index) = edge.type_argument_index {
        details.push(format!("arg={index}"));
    }
    if let Some(type_expression) = &edge.type_expression {
        details.push(format!("type={type_expression}"));
    }
    if edge.is_reference {
        details.push("ref".to_string());
    }
    if edge.is_mutable {
        details.push("mut".to_string());
    }
    if edge.confidence != "high" {
        details.push(format!("confidence={}", edge.confidence));
    }

    if details.is_empty() {
        edge.relationship.clone()
    } else {
        format!("{} {}", edge.relationship, details.join(" "))
    }
}

#[derive(Debug)]
struct BytecodeLoadState {
    key: BytecodeTargetKey,
    package_name: String,
    module_name: String,
    stamp: BytecodeCacheStamp,
    epoch: u64,
}

#[derive(Debug)]
struct BytecodeLoadResult {
    epoch: u64,
    key: BytecodeTargetKey,
    stamp: BytecodeCacheStamp,
    result: Result<BytecodeSession, String>,
}

#[derive(Debug, Clone)]
struct BytecodeCacheEntry {
    stamp: BytecodeCacheStamp,
    session: BytecodeSession,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct BytecodeCacheStamp {
    file_count: u64,
    total_len: u64,
    latest_modified_nanos: u128,
}

#[derive(Debug)]
struct BytecodeOptions {
    context: CliContext,
    key: BytecodeOptionsKey,
    package_name: String,
    targets: Vec<BytecodeTarget>,
}

impl BytecodeOptions {
    fn new(context: CliContext, file: Option<String>, mut targets: Vec<BytecodeTarget>) -> Self {
        targets.sort_by(|left, right| {
            left.file_path
                .cmp(&right.file_path)
                .then_with(|| left.module_name.cmp(&right.module_name))
        });
        let package_name = bytecode_package_name(&context);

        Self {
            key: BytecodeOptionsKey {
                package_root: context.package_root.clone(),
                file,
            },
            context,
            package_name,
            targets,
        }
    }

    fn contains_target_key(&self, key: &BytecodeTargetKey) -> bool {
        self.targets
            .iter()
            .any(|target| BytecodeTargetKey::new(&self.context, target) == *key)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BytecodeOptionsKey {
    package_root: PathBuf,
    file: Option<String>,
}

#[derive(Debug)]
struct BytecodeSelector {
    context: CliContext,
    key: BytecodeOptionsKey,
    package_name: String,
    targets: Vec<BytecodeTarget>,
    selected: usize,
}

impl BytecodeSelector {
    fn new(options: BytecodeOptions) -> Self {
        Self {
            context: options.context,
            key: options.key,
            package_name: options.package_name,
            targets: options.targets,
            selected: 0,
        }
    }

    fn matches(&self, options: &BytecodeOptions) -> bool {
        self.key == options.key && self.targets == options.targets
    }

    fn selected_request(&self) -> Option<BytecodeRequest> {
        self.targets
            .get(self.selected)
            .cloned()
            .map(|target| BytecodeRequest::new(self.context.clone(), target))
    }

    fn handle_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => self.select_previous(),
            KeyCode::Down | KeyCode::Char('j') => self.select_next(),
            KeyCode::PageUp => self.select_previous_page(),
            KeyCode::PageDown => self.select_next_page(),
            KeyCode::Home => self.selected = 0,
            KeyCode::End => {
                self.selected = self.targets.len().saturating_sub(1);
            }
            _ => {}
        }
    }

    fn select_previous(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    fn select_next(&mut self) {
        if self.selected + 1 < self.targets.len() {
            self.selected += 1;
        }
    }

    fn select_previous_page(&mut self) {
        self.selected = self.selected.saturating_sub(PAGE_SIZE);
    }

    fn select_next_page(&mut self) {
        self.selected = self
            .selected
            .saturating_add(PAGE_SIZE)
            .min(self.targets.len().saturating_sub(1));
    }

    fn render(&self, frame: &mut Frame<'_>, area: Rect, palette: ThemePalette, focused: bool) {
        let base_style = Style::default().fg(palette.fg).bg(palette.bg);
        let border_style = Style::default()
            .fg(if focused {
                palette.accent
            } else {
                palette.graph.edge
            })
            .bg(palette.bg);
        let title_style = Style::default()
            .fg(if focused { palette.accent } else { palette.fg })
            .bg(palette.bg)
            .add_modifier(if focused {
                Modifier::BOLD
            } else {
                Modifier::empty()
            });
        let items = self
            .targets
            .iter()
            .map(|target| {
                ListItem::new(Line::from(vec![
                    Span::styled(
                        target.module_name.clone(),
                        Style::default()
                            .fg(palette.accent)
                            .bg(palette.bg)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw("  "),
                    Span::styled(
                        target.file_path.clone(),
                        Style::default().fg(palette.muted).bg(palette.bg),
                    ),
                ]))
                .style(base_style)
            })
            .collect::<Vec<_>>();
        let mut state = ListState::default().with_selected(Some(self.selected));
        let list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(format!("Select Bytecode Module - {}", self.package_name))
                    .style(base_style)
                    .border_style(border_style)
                    .title_style(title_style),
            )
            .style(base_style)
            .highlight_style(
                Style::default()
                    .fg(palette.fg)
                    .bg(palette.selection)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("> ");
        frame.render_stateful_widget(list, area, &mut state);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct BytecodeTargetKey {
    package_root: PathBuf,
    module_name: String,
    source_path: PathBuf,
}

impl BytecodeTargetKey {
    fn new(context: &CliContext, target: &BytecodeTarget) -> Self {
        Self {
            package_root: context.package_root.clone(),
            module_name: target.module_name.clone(),
            source_path: target.source_path.clone(),
        }
    }
}

#[derive(Debug)]
struct BytecodeRequest {
    context: CliContext,
    key: BytecodeTargetKey,
    package_name: String,
}

impl BytecodeRequest {
    fn new(context: CliContext, target: BytecodeTarget) -> Self {
        let package_name = bytecode_package_name(&context);

        Self {
            key: BytecodeTargetKey::new(&context, &target),
            context,
            package_name,
        }
    }
}

fn bytecode_package_name(context: &CliContext) -> String {
    if context.package_path == "." {
        context
            .package_root
            .file_name()
            .and_then(OsStr::to_str)
            .unwrap_or("package")
            .to_string()
    } else {
        context.package_path.clone()
    }
}

#[derive(Debug, Clone)]
struct BytecodeSession {
    key: BytecodeTargetKey,
    package_name: String,
    viewer: OwnedBytecodeView,
    current_line: u16,
    current_column: u16,
    horizontal_scroll: u16,
    viewport_width: u16,
}

impl BytecodeSession {
    fn load(request: BytecodeRequest) -> Result<Self, String> {
        let install_dir =
            tempfile::tempdir().map_err(|error| format!("Could not create build dir: {error}"))?;
        let mut build_config = move_package_alt_compilation::build_config::BuildConfig::default();
        build_config.install_dir = Some(install_dir.path().to_path_buf());
        build_config.silence_warnings = true;
        let env = move_package_alt_compilation::find_env::<sui_package_alt::SuiFlavor>(
            &request.context.package_root,
            &build_config,
        )
        .map_err(|error| format!("Could not resolve Move package environment: {error:#}"))?;
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(|error| format!("Could not create bytecode runtime: {error}"))?;
        let mut writer = Vec::new();
        let package = runtime
            .block_on(async {
                let root_package = build_config
                    .package_loader(&request.context.package_root, &env)
                    .load::<sui_package_alt::SuiFlavor>()
                    .await?;
                move_package_alt_compilation::build_plan::BuildPlan::create(
                    &root_package,
                    &build_config,
                )?
                .compile_with_driver(&mut writer, |compiler| {
                    let (files, units_result) = compiler.build()?;
                    match units_result {
                        Ok((units, _warnings)) => Ok((files, units)),
                        Err(_diagnostics) => Err(io::Error::new(
                            io::ErrorKind::Other,
                            "Compilation failed; fix package errors and try again.",
                        )
                        .into()),
                    }
                })
            })
            .map_err(|error| format!("Could not build package bytecode: {error:#}"))?;
        let compiled_package_name = package.compiled_package_info.package_name.as_str();
        let unit = package
            .get_module_by_name(compiled_package_name, &request.key.module_name)
            .ok()
            .ok_or_else(|| {
                format!(
                    "Built package `{compiled_package_name}` but could not find module `{}`.",
                    request.key.module_name
                )
            })?;
        let viewer = OwnedBytecodeView::new(
            &unit.unit.module,
            unit.unit.source_map.clone(),
            &request.key.source_path,
        )?;

        Ok(Self {
            key: request.key,
            package_name: compiled_package_name.to_string(),
            viewer,
            current_line: 0,
            current_column: 0,
            horizontal_scroll: 0,
            viewport_width: 1,
        })
    }

    fn handle_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Up => self.move_line_up(1),
            KeyCode::Down => self.move_line_down(1),
            KeyCode::PageUp => self.move_line_up(PAGE_SIZE as u16),
            KeyCode::PageDown => self.move_line_down(PAGE_SIZE as u16),
            KeyCode::Left => {
                self.current_column = self.current_column.saturating_sub(1);
                self.ensure_column_visible();
            }
            KeyCode::Right => {
                self.current_column = self
                    .viewer
                    .bound_column(self.current_line, self.current_column.saturating_add(1));
                self.ensure_column_visible();
            }
            KeyCode::Home => {
                self.current_column = 0;
                self.ensure_column_visible();
            }
            KeyCode::End => {
                self.current_column = self.viewer.bound_column(self.current_line, u16::MAX);
                self.ensure_column_visible();
            }
            _ => {}
        }
    }

    fn scroll_vertical(&mut self, down: bool, amount: u16) {
        if down {
            self.move_line_down(amount);
        } else {
            self.move_line_up(amount);
        }
    }

    fn scroll_horizontal(&mut self, right: bool, amount: u16) {
        if right {
            self.horizontal_scroll = self
                .horizontal_scroll
                .saturating_add(amount)
                .min(self.max_horizontal_scroll());
        } else {
            self.horizontal_scroll = self.horizontal_scroll.saturating_sub(amount);
        }
    }

    fn move_line_up(&mut self, amount: u16) {
        self.current_line = self.current_line.saturating_sub(amount);
        self.current_column = self
            .viewer
            .bound_column(self.current_line, self.current_column);
        self.ensure_column_visible();
    }

    fn move_line_down(&mut self, amount: u16) {
        self.current_line = self
            .viewer
            .bound_line(self.current_line.saturating_add(amount));
        self.current_column = self
            .viewer
            .bound_column(self.current_line, self.current_column);
        self.ensure_column_visible();
    }

    fn set_viewport_width(&mut self, width: u16) {
        self.viewport_width = width.max(1);
        self.horizontal_scroll = self.horizontal_scroll.min(self.max_horizontal_scroll());
    }

    fn max_horizontal_scroll(&self) -> u16 {
        self.viewer
            .max_bytecode_width()
            .saturating_add(1)
            .saturating_sub(self.viewport_width)
    }

    fn ensure_column_visible(&mut self) {
        if self.current_column < self.horizontal_scroll {
            self.horizontal_scroll = self.current_column;
        } else if self.current_column >= self.horizontal_scroll.saturating_add(self.viewport_width)
        {
            self.horizontal_scroll = self
                .current_column
                .saturating_add(1)
                .saturating_sub(self.viewport_width);
        }
        self.horizontal_scroll = self.horizontal_scroll.min(self.max_horizontal_scroll());
    }

    fn render(
        &mut self,
        frame: &mut Frame<'_>,
        area: Rect,
        palette: ThemePalette,
        focused: bool,
        light_theme: bool,
    ) {
        let panes = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
            .split(area);
        let inner_height = panes[0].height.saturating_sub(2);
        self.set_viewport_width(panes[0].width.saturating_sub(2));
        let scroll = if inner_height == 0 {
            0
        } else {
            self.current_line.saturating_sub(inner_height / 2)
        };
        let base_style = Style::default().fg(palette.fg).bg(palette.bg);
        let border_style = Style::default()
            .fg(if focused {
                palette.accent
            } else {
                palette.graph.edge
            })
            .bg(palette.bg);
        let title_style = Style::default()
            .fg(if focused { palette.accent } else { palette.fg })
            .bg(palette.bg)
            .add_modifier(if focused {
                Modifier::BOLD
            } else {
                Modifier::empty()
            });
        let selected_style = Style::default()
            .fg(palette.fg)
            .bg(palette.selection)
            .add_modifier(Modifier::BOLD);
        let bytecode_lines =
            self.viewer
                .bytecode_lines(scroll, inner_height, self.current_line, selected_style);
        let source_lines =
            self.viewer
                .source_lines(self.current_line, self.current_column, palette, light_theme);

        let bytecode = Paragraph::new(bytecode_lines)
            .style(base_style)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(format!(
                        "Bytecode - {}::{}",
                        self.package_name, self.key.module_name
                    ))
                    .style(base_style)
                    .border_style(border_style)
                    .title_style(title_style),
            )
            .scroll((scroll, self.horizontal_scroll));
        frame.render_widget(bytecode, panes[0]);

        let source = Paragraph::new(source_lines).style(base_style).block(
            Block::default()
                .borders(Borders::ALL)
                .title("Source Code")
                .style(base_style)
                .border_style(border_style)
                .title_style(title_style),
        );
        frame.render_widget(source, panes[1]);

        if focused && inner_height > 0 {
            let y = self
                .current_line
                .saturating_sub(scroll)
                .min(inner_height - 1);
            let x = self.current_column.saturating_sub(self.horizontal_scroll);
            if self.current_column >= self.horizontal_scroll && x < self.viewport_width {
                frame.set_cursor_position(Position::new(panes[0].x + 1 + x, panes[0].y + 1 + y));
            }
        }
    }
}

#[derive(Debug, Clone)]
struct OwnedBytecodeView {
    bytecode_lines: Vec<String>,
    line_map: HashMap<usize, BytecodeLineInfo>,
    source_code: String,
    source_map: SourceMap,
}

impl OwnedBytecodeView {
    fn new(
        module: &CompiledModule,
        source_map: SourceMap,
        source_path: &Path,
    ) -> Result<Self, String> {
        let source_code = fs::read_to_string(source_path)
            .map_err(|error| format!("Could not read {}: {error}", source_path.display()))?;
        if !source_map.check(&source_code) {
            return Err(format!(
                "Source map for {} is out of sync with the source file.",
                source_path.display()
            ));
        }
        let source_mapping = SourceMapping::new(source_map.clone(), module);
        let options = DisassemblerOptions {
            print_code: true,
            print_basic_blocks: true,
            ..Default::default()
        };
        let disassembled = Disassembler::new(source_mapping, options)
            .disassemble()
            .map_err(|error| format!("Could not disassemble module bytecode: {error}"))?;
        let bytecode_lines = disassembled
            .lines()
            .map(|line| line.replace('\t', "    "))
            .collect::<Vec<_>>();
        let line_map = build_bytecode_line_map(module, &bytecode_lines)?;

        Ok(Self {
            bytecode_lines,
            line_map,
            source_code,
            source_map,
        })
    }

    fn bytecode_lines(
        &self,
        scroll: u16,
        height: u16,
        current_line: u16,
        selected_style: Style,
    ) -> Vec<Line<'static>> {
        self.bytecode_lines
            .iter()
            .skip(scroll as usize)
            .take(height as usize)
            .enumerate()
            .map(|(visible_index, line)| {
                let line_index = scroll as usize + visible_index;
                if line_index == current_line as usize {
                    Line::styled(line.clone(), selected_style)
                } else {
                    Line::from(line.clone())
                }
            })
            .collect()
    }

    fn max_bytecode_width(&self) -> u16 {
        self.bytecode_lines
            .iter()
            .map(|line| usize_to_u16_saturating(char_len(line)))
            .max()
            .unwrap_or(0)
    }

    fn source_lines(
        &self,
        line_number: u16,
        _column_number: u16,
        palette: ThemePalette,
        light_theme: bool,
    ) -> Vec<Line<'static>> {
        let base_style = Style::default().fg(palette.syntax.text).bg(palette.bg);
        let highlight_style = Style::default()
            .fg(if light_theme { palette.bg } else { palette.fg })
            .bg(palette.warning)
            .add_modifier(Modifier::BOLD);
        let Some(info) = self.line_map.get(&(line_number as usize)) else {
            return self
                .source_code
                .lines()
                .map(|line| Line::styled(line.to_string(), base_style))
                .collect();
        };
        let Ok(location) = self
            .source_map
            .get_code_location(info.function_index, info.code_offset)
        else {
            return vec![Line::styled(
                "No source location is available for this bytecode offset.",
                Style::default().fg(palette.muted).bg(palette.bg),
            )];
        };
        let start = location.start() as usize;
        let end = location.end() as usize;
        let source_len = self.source_code.len();
        let context_start = start.saturating_sub(1000);
        let context_end = end.saturating_add(1000).min(source_len);

        styled_text_segments([
            (&self.source_code[context_start..start], base_style),
            (&self.source_code[start..end], highlight_style),
            (&self.source_code[end..context_end], base_style),
        ])
    }

    fn bound_line(&self, line_number: u16) -> u16 {
        let last = self.bytecode_lines.len().saturating_sub(1) as u16;
        line_number.min(last)
    }

    fn bound_column(&self, line_number: u16, column_number: u16) -> u16 {
        let line = self
            .bytecode_lines
            .get(line_number as usize)
            .map(String::as_str)
            .unwrap_or_default();
        column_number.min(char_len(line) as u16)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BytecodeLineInfo {
    function_index: FunctionDefinitionIndex,
    code_offset: CodeOffset,
}

fn build_bytecode_line_map(
    module: &CompiledModule,
    lines: &[String],
) -> Result<HashMap<usize, BytecodeLineInfo>, String> {
    let offset_regex =
        Regex::new(r"^(\d+):.*").map_err(|error| format!("Invalid offset regex: {error}"))?;
    let function_regex =
        Regex::new(r"^(?:public(?:\(\w+\))?|native|entry)?\s*(\w+)\s*(?:<.*>)?\s*\(.*\).*\{")
            .map_err(|error| format!("Invalid function regex: {error}"))?;
    let function_def_for_name = module
        .function_defs()
        .iter()
        .enumerate()
        .map(|(index, definition)| {
            (
                module
                    .identifier_at(module.function_handle_at(definition.function).name)
                    .to_string(),
                FunctionDefinitionIndex(index as u16),
            )
        })
        .collect::<HashMap<_, _>>();
    let mut current_function = None;
    let mut line_map = HashMap::new();

    for (line_index, line) in lines.iter().map(|line| line.trim()).enumerate() {
        if let Some(capture) = function_regex.captures(line) {
            current_function = capture
                .get(1)
                .and_then(|name| function_def_for_name.get(name.as_str()).copied());
        }

        let Some(function_index) = current_function else {
            continue;
        };
        let Some(offset) = offset_regex
            .captures(line)
            .and_then(|capture| capture.get(1))
            .and_then(|offset| offset.as_str().parse::<CodeOffset>().ok())
        else {
            continue;
        };
        line_map.insert(
            line_index,
            BytecodeLineInfo {
                function_index,
                code_offset: offset,
            },
        );
    }

    Ok(line_map)
}

fn styled_text_segments<const N: usize>(segments: [(&str, Style); N]) -> Vec<Line<'static>> {
    let mut lines = vec![Vec::new()];
    for (text, style) in segments {
        append_styled_text(&mut lines, text, style);
    }
    lines.into_iter().map(Line::from).collect()
}

fn append_styled_text(lines: &mut Vec<Vec<Span<'static>>>, text: &str, style: Style) {
    for (index, part) in text.split('\n').enumerate() {
        if index > 0 {
            lines.push(Vec::new());
        }
        if !part.is_empty() {
            lines
                .last_mut()
                .expect("styled text has at least one line")
                .push(Span::styled(part.to_string(), style));
        }
    }
}

fn nearest_move_package_root(path: &Path, project_root: &Path) -> Option<PathBuf> {
    let project_root = project_root.canonicalize().ok()?;
    let mut current = if path.is_dir() {
        path.to_path_buf()
    } else {
        path.parent()?.to_path_buf()
    }
    .canonicalize()
    .ok()?;

    loop {
        if current.join("Move.toml").is_file() {
            return Some(current);
        }
        if current == project_root || !current.pop() {
            return None;
        }
        if !current.starts_with(&project_root) {
            return None;
        }
    }
}

fn relative_path_label(root: &Path, path: &Path) -> String {
    let relative = path.strip_prefix(root).unwrap_or(path);

    if relative.as_os_str().is_empty() {
        ".".to_string()
    } else {
        normalized_path_string(relative)
    }
}

fn normalized_path_string(path: impl AsRef<Path>) -> String {
    path.as_ref()
        .components()
        .filter_map(|component| match component {
            std::path::Component::Normal(value) => Some(value.to_string_lossy().into_owned()),
            std::path::Component::CurDir => None,
            other => Some(other.as_os_str().to_string_lossy().into_owned()),
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn bytecode_cache_stamp(package_root: &Path) -> BytecodeCacheStamp {
    let mut stamp = BytecodeCacheStamp::default();
    visit_bytecode_cache_files(package_root, &mut stamp);
    stamp
}

fn visit_bytecode_cache_files(path: &Path, stamp: &mut BytecodeCacheStamp) {
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return;
    };

    if metadata.is_file() {
        if !bytecode_cache_relevant_file(path) {
            return;
        }
        stamp.file_count = stamp.file_count.saturating_add(1);
        stamp.total_len = stamp.total_len.wrapping_add(metadata.len());
        let modified = metadata
            .modified()
            .ok()
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
            .map(|duration| duration.as_nanos())
            .unwrap_or_default();
        stamp.latest_modified_nanos = stamp.latest_modified_nanos.max(modified);
        return;
    }

    if !metadata.is_dir() || bytecode_cache_skipped_dir(path) {
        return;
    }

    let Ok(entries) = fs::read_dir(path) else {
        return;
    };
    for entry in entries.flatten() {
        visit_bytecode_cache_files(&entry.path(), stamp);
    }
}

fn bytecode_cache_relevant_file(path: &Path) -> bool {
    path.extension() == Some(OsStr::new("move"))
        || path
            .file_name()
            .and_then(OsStr::to_str)
            .is_some_and(|name| matches!(name, "Move.toml" | "Move.lock"))
}

fn bytecode_cache_skipped_dir(path: &Path) -> bool {
    path.file_name()
        .and_then(OsStr::to_str)
        .is_some_and(|name| {
            matches!(
                name,
                ".git" | ".peregrine" | ".peregrine-dev" | "build" | "target"
            )
        })
}

fn hidden_inspector_focus_order() -> &'static [FocusPane] {
    const WITHOUT_INSPECTOR: [FocusPane; 4] = [
        FocusPane::Explorer,
        FocusPane::Editor,
        FocusPane::Input,
        FocusPane::Tabs,
    ];

    &WITHOUT_INSPECTOR
}

fn rect_contains(rect: Rect, x: u16, y: u16) -> bool {
    x >= rect.x && x < rect.right() && y >= rect.y && y < rect.bottom()
}

fn inner_rect(rect: Rect) -> Rect {
    Rect {
        x: rect.x.saturating_add(1),
        y: rect.y.saturating_add(1),
        width: rect.width.saturating_sub(2),
        height: rect.height.saturating_sub(2),
    }
}

fn usize_to_u16_saturating(value: usize) -> u16 {
    value.min(usize::from(u16::MAX)) as u16
}

fn centered_rect(area: Rect, max_width: u16, max_height: u16) -> Rect {
    let width = area.width.min(max_width).max(1);
    let height = area.height.min(max_height).max(1);
    Rect {
        x: area.x + area.width.saturating_sub(width) / 2,
        y: area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    }
}

fn startup_option_line(
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

fn is_quit_key(key: KeyEvent) -> bool {
    key.modifiers == KeyModifiers::CONTROL
        && matches!(key.code, KeyCode::Char('c') | KeyCode::Char('q'))
}

fn default_package_name(root: &Path) -> String {
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

fn package_name_error(package_name: &str) -> Option<String> {
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

fn render_cli_step_summary(step: &CliStep) -> String {
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

fn startup_failure_load_report(package_root: PathBuf, message: String) -> PackageLoadReport {
    let reason = message.clone();
    PackageLoadReport {
        package_root,
        build: failed_startup_step("build", message),
        test: CliStep::skipped("test", "package loading could not start"),
        scanners: PackageScannerReport {
            compiler_unit_tests: ScannerResult::Unavailable {
                reason: reason.clone(),
            },
            compiler_movy_invariant_tests: ScannerResult::Unavailable {
                reason: reason.clone(),
            },
            compiler_fuzz_tests: ScannerResult::Unavailable {
                reason: reason.clone(),
            },
            compiler_formal_verification: ScannerResult::Unavailable {
                reason: reason.clone(),
            },
            heuristic_unit_tests: ScannerResult::Unavailable {
                reason: reason.clone(),
            },
            heuristic_movy_invariant_tests: ScannerResult::Unavailable {
                reason: reason.clone(),
            },
            heuristic_fuzz_tests: ScannerResult::Unavailable {
                reason: reason.clone(),
            },
            heuristic_formal_verification: ScannerResult::Unavailable { reason },
        },
    }
}

fn package_load_status(report: &PackageLoadReport) -> String {
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

fn package_load_status_lines(report: &PackageLoadReport, app: &App) -> Vec<Line<'static>> {
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

fn package_load_spinner(started_at: Instant) -> &'static str {
    const FRAMES: [&str; 4] = ["-", "\\", "|", "/"];
    let frame = (started_at.elapsed().as_millis() / 125) as usize % FRAMES.len();
    FRAMES[frame]
}

fn format_elapsed(duration: Duration) -> String {
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

fn focused_title(title: &str, focused: bool) -> String {
    if focused {
        format!("* {title}")
    } else {
        title.to_string()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TuiSettings {
    pub editor_mode: EditorMode,
    pub theme: Theme,
}

impl Default for TuiSettings {
    fn default() -> Self {
        Self {
            editor_mode: EditorMode::Standard,
            theme: Theme::default(),
        }
    }
}

pub fn configured_tui_settings() -> TuiSettings {
    match peregrine_utils_home_dir::find_peregrine_home() {
        Ok(home) => load_tui_settings_from_home(home.as_path()),
        Err(_) => TuiSettings::default(),
    }
}

pub fn load_tui_settings_from_home(home: &Path) -> TuiSettings {
    let config_path = home.join(CONFIG_TOML_FILE);
    let Ok(contents) = fs::read_to_string(config_path) else {
        return TuiSettings::default();
    };
    let Ok(config) = toml::from_str::<ConfigToml>(&contents) else {
        return TuiSettings::default();
    };

    let Some(tui) = config.tui.as_ref() else {
        return TuiSettings::default();
    };

    let editor_mode = if tui.vim_mode_default {
        EditorMode::Vim
    } else {
        EditorMode::Standard
    };
    let theme = tui
        .theme
        .as_deref()
        .and_then(|name| name.parse::<ThemeName>().ok())
        .map(Theme::new)
        .unwrap_or_default();

    TuiSettings { editor_mode, theme }
}

pub fn configured_editor_mode() -> EditorMode {
    configured_tui_settings().editor_mode
}

pub fn load_editor_mode_from_home(home: &Path) -> EditorMode {
    load_tui_settings_from_home(home).editor_mode
}

pub fn configured_theme() -> Theme {
    configured_tui_settings().theme
}

pub fn load_theme_from_home(home: &Path) -> Theme {
    load_tui_settings_from_home(home).theme
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExplorerEntry {
    path: PathBuf,
    name: String,
    is_dir: bool,
    depth: usize,
    expanded: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ExplorerAction {
    OpenFile(PathBuf),
    ToggledDirectory,
    None,
}

pub struct Explorer {
    root: PathBuf,
    expanded: BTreeSet<PathBuf>,
    visible: Vec<ExplorerEntry>,
    selected: usize,
}

impl Explorer {
    pub fn new(root: impl AsRef<Path>) -> io::Result<Self> {
        let root = root.as_ref().canonicalize()?;
        let mut expanded = BTreeSet::new();
        expanded.insert(root.clone());
        let mut explorer = Self {
            root,
            expanded,
            visible: Vec::new(),
            selected: 0,
        };
        explorer.refresh();
        Ok(explorer)
    }

    pub fn visible_entries(&self) -> &[ExplorerEntry] {
        &self.visible
    }

    pub fn selected(&self) -> usize {
        self.selected
    }

    pub fn selected_path(&self) -> Option<&Path> {
        self.visible
            .get(self.selected)
            .map(|entry| entry.path.as_path())
    }

    pub fn select_next(&mut self) {
        if self.selected + 1 < self.visible.len() {
            self.selected += 1;
        }
    }

    pub fn select_previous(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    fn activate_selected(&mut self) -> ExplorerAction {
        let Some(entry) = self.visible.get(self.selected).cloned() else {
            return ExplorerAction::None;
        };
        if entry.is_dir {
            if self.expanded.contains(&entry.path) {
                self.expanded.remove(&entry.path);
            } else {
                self.expanded.insert(entry.path);
            }
            self.refresh();
            ExplorerAction::ToggledDirectory
        } else {
            ExplorerAction::OpenFile(entry.path)
        }
    }

    fn refresh(&mut self) {
        self.visible.clear();
        self.push_visible(self.root.clone(), 0);
        if self.visible.is_empty() {
            self.selected = 0;
        } else if self.selected >= self.visible.len() {
            self.selected = self.visible.len() - 1;
        }
    }

    fn push_visible(&mut self, path: PathBuf, depth: usize) {
        let is_dir = path.is_dir();
        let expanded = is_dir && self.expanded.contains(&path);
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .map(str::to_string)
            .unwrap_or_else(|| path.display().to_string());
        self.visible.push(ExplorerEntry {
            path: path.clone(),
            name,
            is_dir,
            depth,
            expanded,
        });
        if !expanded {
            return;
        }
        for child in sorted_children(&path) {
            self.push_visible(child.path, depth + 1);
        }
    }
}

struct ChildEntry {
    path: PathBuf,
    name: String,
    is_dir: bool,
}

fn sorted_children(path: &Path) -> Vec<ChildEntry> {
    let Ok(read_dir) = fs::read_dir(path) else {
        return Vec::new();
    };
    let mut children = read_dir
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let file_type = entry.file_type().ok()?;
            let name = entry.file_name().to_string_lossy().into_owned();
            Some(ChildEntry {
                path: entry.path(),
                name,
                is_dir: file_type.is_dir(),
            })
        })
        .collect::<Vec<_>>();
    children.sort_by(|left, right| match (left.is_dir, right.is_dir) {
        (true, false) => Ordering::Less,
        (false, true) => Ordering::Greater,
        _ => left.name.cmp(&right.name),
    });
    children
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Cursor {
    row: usize,
    col: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct EditorSnapshot {
    lines: Vec<String>,
    cursor: Cursor,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct EditorRenderCache {
    path: Option<PathBuf>,
    source: String,
    theme: ThemeName,
    markdown_preview: bool,
    width: usize,
    root: PathBuf,
    document: RenderedWorkbenchDocument,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PendingVimCommand {
    Delete,
    Yank,
}

pub struct EditorBuffer {
    path: Option<PathBuf>,
    lines: Vec<String>,
    cursor: Cursor,
    scroll: usize,
    horizontal_scroll: usize,
    dirty: bool,
    undo_stack: Vec<EditorSnapshot>,
    yank: Vec<String>,
    pending_vim: Option<PendingVimCommand>,
    viewport_height: usize,
    viewport_width: usize,
}

impl EditorBuffer {
    pub fn new_empty() -> Self {
        Self {
            path: None,
            lines: vec![String::new()],
            cursor: Cursor { row: 0, col: 0 },
            scroll: 0,
            horizontal_scroll: 0,
            dirty: false,
            undo_stack: Vec::new(),
            yank: Vec::new(),
            pending_vim: None,
            viewport_height: 1,
            viewport_width: 1,
        }
    }

    pub fn open_file(&mut self, path: &Path) -> io::Result<()> {
        let contents = fs::read_to_string(path)?;
        self.path = Some(path.to_path_buf());
        self.lines = split_lines(&contents);
        self.cursor = Cursor { row: 0, col: 0 };
        self.scroll = 0;
        self.horizontal_scroll = 0;
        self.dirty = false;
        self.undo_stack.clear();
        self.pending_vim = None;
        Ok(())
    }

    pub fn save(&mut self) -> io::Result<()> {
        let Some(path) = &self.path else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "no file is open",
            ));
        };
        fs::write(path, self.text())?;
        self.dirty = false;
        Ok(())
    }

    pub fn reload(&mut self) -> io::Result<()> {
        let Some(path) = self.path.clone() else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "no file is open",
            ));
        };
        self.open_file(&path)
    }

    pub fn text(&self) -> String {
        self.lines.join("\n")
    }

    fn line_count(&self) -> usize {
        self.lines.len().max(1)
    }

    fn line_number_digit_width(&self) -> usize {
        self.line_count().to_string().len().max(2)
    }

    fn line_number_gutter_width(&self) -> usize {
        self.line_number_digit_width() + 1
    }

    fn line_numbers_text(&self) -> String {
        let width = self.line_number_digit_width();
        (1..=self.line_count())
            .map(|line| format!("{line:>width$} "))
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn display_name(&self) -> String {
        self.path
            .as_ref()
            .and_then(|path| path.file_name())
            .and_then(|name| name.to_str())
            .map(str::to_string)
            .unwrap_or_else(|| String::from("No file"))
    }

    fn set_viewport_size(&mut self, height: usize, width: usize) {
        self.viewport_height = height.max(1);
        self.viewport_width = width.max(1);
        self.clamp_scrolls();
    }

    fn handle_standard_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char(c) if editable_char_modifiers(key.modifiers) => self.insert_char(c),
            KeyCode::Enter => self.insert_newline(),
            KeyCode::Backspace => self.backspace(),
            KeyCode::Delete => self.delete_char(),
            KeyCode::Tab => self.insert_char('\t'),
            KeyCode::Left => self.move_left(),
            KeyCode::Right => self.move_right(),
            KeyCode::Up => self.move_up(),
            KeyCode::Down => self.move_down(),
            KeyCode::Home => self.move_line_start(),
            KeyCode::End => self.move_line_end(),
            KeyCode::PageUp => self.page_up(),
            KeyCode::PageDown => self.page_down(),
            _ => {}
        }
    }

    fn insert_char(&mut self, c: char) {
        self.record_undo();
        let row = self.cursor.row;
        let col = self.cursor.col;
        let byte = char_to_byte_index(&self.lines[row], col);
        self.lines[row].insert(byte, c);
        self.cursor.col += 1;
        self.mark_dirty();
    }

    fn insert_newline(&mut self) {
        self.record_undo();
        let row = self.cursor.row;
        let byte = char_to_byte_index(&self.lines[row], self.cursor.col);
        let rest = self.lines[row].split_off(byte);
        self.lines.insert(row + 1, rest);
        self.cursor.row += 1;
        self.cursor.col = 0;
        self.mark_dirty();
    }

    fn backspace(&mut self) {
        if self.cursor.col == 0 && self.cursor.row == 0 {
            return;
        }
        self.record_undo();
        if self.cursor.col > 0 {
            let row = self.cursor.row;
            let end = char_to_byte_index(&self.lines[row], self.cursor.col);
            let start = char_to_byte_index(&self.lines[row], self.cursor.col - 1);
            self.lines[row].replace_range(start..end, "");
            self.cursor.col -= 1;
        } else {
            let row = self.cursor.row;
            let removed = self.lines.remove(row);
            self.cursor.row -= 1;
            self.cursor.col = char_len(&self.lines[self.cursor.row]);
            self.lines[self.cursor.row].push_str(&removed);
        }
        self.mark_dirty();
    }

    fn delete_char(&mut self) {
        let row = self.cursor.row;
        let line_len = char_len(&self.lines[row]);
        if self.cursor.col >= line_len {
            if row + 1 >= self.lines.len() {
                return;
            }
            self.record_undo();
            let next = self.lines.remove(row + 1);
            self.lines[row].push_str(&next);
            self.mark_dirty();
            return;
        }

        self.record_undo();
        let start = char_to_byte_index(&self.lines[row], self.cursor.col);
        let end = char_to_byte_index(&self.lines[row], self.cursor.col + 1);
        self.lines[row].replace_range(start..end, "");
        self.mark_dirty();
    }

    fn open_line_below(&mut self) {
        self.record_undo();
        let row = self.cursor.row + 1;
        self.lines.insert(row, String::new());
        self.cursor = Cursor { row, col: 0 };
        self.mark_dirty();
    }

    fn open_line_above(&mut self) {
        self.record_undo();
        let row = self.cursor.row;
        self.lines.insert(row, String::new());
        self.cursor = Cursor { row, col: 0 };
        self.mark_dirty();
    }

    fn delete_current_line(&mut self) {
        if self.lines.is_empty() {
            return;
        }
        self.record_undo();
        let removed = self.lines.remove(self.cursor.row);
        self.yank = vec![removed];
        if self.lines.is_empty() {
            self.lines.push(String::new());
        }
        self.cursor.row = self.cursor.row.min(self.lines.len() - 1);
        self.cursor.col = 0;
        self.mark_dirty();
    }

    fn yank_current_line(&mut self) {
        if let Some(line) = self.lines.get(self.cursor.row) {
            self.yank = vec![line.clone()];
        }
    }

    fn paste_after(&mut self) {
        if self.yank.is_empty() {
            return;
        }
        self.record_undo();
        let mut insert_at = self.cursor.row + 1;
        for line in &self.yank {
            self.lines.insert(insert_at, line.clone());
            insert_at += 1;
        }
        self.cursor.row += 1;
        self.cursor.col = 0;
        self.mark_dirty();
    }

    fn undo(&mut self) {
        let Some(snapshot) = self.undo_stack.pop() else {
            return;
        };
        self.lines = snapshot.lines;
        self.cursor = snapshot.cursor;
        self.dirty = true;
        self.ensure_cursor_in_bounds();
    }

    fn move_left(&mut self) {
        if self.cursor.col > 0 {
            self.cursor.col -= 1;
        } else if self.cursor.row > 0 {
            self.cursor.row -= 1;
            self.cursor.col = char_len(&self.lines[self.cursor.row]);
        }
        self.ensure_cursor_visible();
    }

    fn move_right(&mut self) {
        let line_len = char_len(&self.lines[self.cursor.row]);
        if self.cursor.col < line_len {
            self.cursor.col += 1;
        } else if self.cursor.row + 1 < self.lines.len() {
            self.cursor.row += 1;
            self.cursor.col = 0;
        }
        self.ensure_cursor_visible();
    }

    fn move_up(&mut self) {
        self.cursor.row = self.cursor.row.saturating_sub(1);
        self.ensure_cursor_in_bounds();
        self.ensure_cursor_visible();
    }

    fn move_down(&mut self) {
        if self.cursor.row + 1 < self.lines.len() {
            self.cursor.row += 1;
        }
        self.ensure_cursor_in_bounds();
        self.ensure_cursor_visible();
    }

    fn move_line_start(&mut self) {
        self.cursor.col = 0;
        self.ensure_cursor_visible();
    }

    fn move_line_end(&mut self) {
        self.cursor.col = char_len(&self.lines[self.cursor.row]);
        self.ensure_cursor_visible();
    }

    fn set_cursor_from_view_position(&mut self, row: usize, col: usize) {
        if self.lines.is_empty() {
            self.lines.push(String::new());
        }
        self.cursor.row = (self.scroll + row).min(self.lines.len() - 1);
        self.cursor.col =
            (self.horizontal_scroll + col).min(char_len(&self.lines[self.cursor.row]));
        self.ensure_cursor_visible();
    }

    fn scroll_vertical(&mut self, down: bool, amount: usize) {
        if down {
            self.scroll = self
                .scroll
                .saturating_add(amount)
                .min(self.max_vertical_scroll());
        } else {
            self.scroll = self.scroll.saturating_sub(amount);
        }
    }

    fn scroll_horizontal(&mut self, right: bool, amount: usize) {
        if right {
            self.horizontal_scroll = self
                .horizontal_scroll
                .saturating_add(amount)
                .min(self.max_horizontal_scroll());
        } else {
            self.horizontal_scroll = self.horizontal_scroll.saturating_sub(amount);
        }
    }

    fn page_up(&mut self) {
        self.cursor.row = self.cursor.row.saturating_sub(PAGE_SIZE);
        self.ensure_cursor_in_bounds();
        self.ensure_cursor_visible();
    }

    fn page_down(&mut self) {
        self.cursor.row = (self.cursor.row + PAGE_SIZE).min(self.lines.len() - 1);
        self.ensure_cursor_in_bounds();
        self.ensure_cursor_visible();
    }

    fn record_undo(&mut self) {
        if self.undo_stack.len() == UNDO_LIMIT {
            self.undo_stack.remove(0);
        }
        self.undo_stack.push(EditorSnapshot {
            lines: self.lines.clone(),
            cursor: self.cursor,
        });
    }

    fn mark_dirty(&mut self) {
        self.dirty = true;
        self.pending_vim = None;
        self.ensure_cursor_in_bounds();
        self.ensure_cursor_visible();
    }

    fn ensure_cursor_in_bounds(&mut self) {
        if self.lines.is_empty() {
            self.lines.push(String::new());
        }
        self.cursor.row = self.cursor.row.min(self.lines.len() - 1);
        self.cursor.col = self.cursor.col.min(char_len(&self.lines[self.cursor.row]));
    }

    fn ensure_cursor_visible(&mut self) {
        if self.cursor.row < self.scroll {
            self.scroll = self.cursor.row;
        } else if self.cursor.row >= self.scroll + self.viewport_height {
            self.scroll = self.cursor.row + 1 - self.viewport_height;
        }
        if self.cursor.col < self.horizontal_scroll {
            self.horizontal_scroll = self.cursor.col;
        } else if self.cursor.col >= self.horizontal_scroll + self.viewport_width {
            self.horizontal_scroll = self.cursor.col + 1 - self.viewport_width;
        }
        self.clamp_scrolls();
    }

    fn clamp_scrolls(&mut self) {
        self.scroll = self.scroll.min(self.max_vertical_scroll());
        self.horizontal_scroll = self.horizontal_scroll.min(self.max_horizontal_scroll());
    }

    fn max_vertical_scroll(&self) -> usize {
        self.lines.len().saturating_sub(self.viewport_height)
    }

    fn max_horizontal_scroll(&self) -> usize {
        self.lines
            .iter()
            .map(|line| char_len(line))
            .max()
            .unwrap_or(0)
            .saturating_add(1)
            .saturating_sub(self.viewport_width)
    }
}

fn split_lines(contents: &str) -> Vec<String> {
    let mut lines = contents
        .split('\n')
        .map(|line| line.strip_suffix('\r').unwrap_or(line).to_string())
        .collect::<Vec<_>>();
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

fn editable_char_modifiers(modifiers: KeyModifiers) -> bool {
    !modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER)
}

fn char_len(value: &str) -> usize {
    value.chars().count()
}

fn char_to_byte_index(value: &str, char_index: usize) -> usize {
    value
        .char_indices()
        .nth(char_index)
        .map(|(byte_index, _)| byte_index)
        .unwrap_or(value.len())
}

#[derive(Debug, Clone)]
pub struct CommandInput {
    text: String,
    cursor: usize,
    scroll: usize,
    viewport_width: usize,
}

impl Default for CommandInput {
    fn default() -> Self {
        Self {
            text: String::new(),
            cursor: 0,
            scroll: 0,
            viewport_width: 1,
        }
    }
}

impl CommandInput {
    fn from_text(text: impl Into<String>) -> Self {
        let text = text.into();
        let cursor = char_len(&text);
        let mut input = Self {
            text,
            cursor,
            scroll: 0,
            viewport_width: 1,
        };
        input.ensure_cursor_visible();
        input
    }

    fn handle_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char(c) if editable_char_modifiers(key.modifiers) => self.insert_char(c),
            KeyCode::Backspace => self.backspace(),
            KeyCode::Delete => self.delete_char(),
            KeyCode::Left => self.cursor = self.cursor.saturating_sub(1),
            KeyCode::Right => self.cursor = (self.cursor + 1).min(char_len(&self.text)),
            KeyCode::Home => self.cursor = 0,
            KeyCode::End => self.cursor = char_len(&self.text),
            _ => {}
        }
        self.ensure_cursor_visible();
    }

    fn set_viewport_width(&mut self, width: usize) {
        self.viewport_width = width.max(1);
        self.scroll = self.scroll.min(self.max_scroll());
    }

    fn scroll_horizontal(&mut self, right: bool, amount: usize) {
        if right {
            self.scroll = self.scroll.saturating_add(amount).min(self.max_scroll());
        } else {
            self.scroll = self.scroll.saturating_sub(amount);
        }
    }

    fn insert_char(&mut self, c: char) {
        let byte = char_to_byte_index(&self.text, self.cursor);
        self.text.insert(byte, c);
        self.cursor += 1;
    }

    fn backspace(&mut self) {
        if self.cursor == 0 {
            return;
        }
        let end = char_to_byte_index(&self.text, self.cursor);
        let start = char_to_byte_index(&self.text, self.cursor - 1);
        self.text.replace_range(start..end, "");
        self.cursor -= 1;
    }

    fn delete_char(&mut self) {
        if self.cursor >= char_len(&self.text) {
            return;
        }
        let start = char_to_byte_index(&self.text, self.cursor);
        let end = char_to_byte_index(&self.text, self.cursor + 1);
        self.text.replace_range(start..end, "");
    }

    fn set_cursor_column(&mut self, col: usize) {
        self.cursor = (self.scroll + col).min(char_len(&self.text));
        self.ensure_cursor_visible();
    }

    fn take_text(&mut self) -> String {
        self.cursor = 0;
        self.scroll = 0;
        std::mem::take(&mut self.text)
    }

    fn ensure_cursor_visible(&mut self) {
        if self.cursor < self.scroll {
            self.scroll = self.cursor;
        } else if self.cursor >= self.scroll + self.viewport_width {
            self.scroll = self.cursor + 1 - self.viewport_width;
        }
        self.scroll = self.scroll.min(self.max_scroll());
    }

    fn max_scroll(&self) -> usize {
        char_len(&self.text)
            .saturating_add(1)
            .saturating_sub(self.viewport_width)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::crossterm::event::KeyModifiers;
    use serde_json::Value;
    use std::collections::BTreeMap;
    use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};

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
    fn no_args_dispatch_to_workbench() {
        assert_eq!(
            classify_top_level_args(Vec::new()),
            TopLevelDispatch::Workbench(None)
        );
    }

    #[test]
    fn agent_subcommand_dispatches_to_agent_parser() {
        assert_eq!(
            classify_top_level_args(vec![
                OsString::from("agent"),
                OsString::from("--model"),
                OsString::from("gpt-5"),
            ]),
            TopLevelDispatch::Agent(vec![OsString::from("--model"), OsString::from("gpt-5")])
        );
    }

    #[test]
    fn workflow_command_dispatches_to_cli() {
        assert_eq!(
            classify_top_level_args(vec![OsString::from("build")]),
            TopLevelDispatch::CliOrHelper(vec![OsString::from("build")])
        );
    }

    #[test]
    fn single_directory_arg_dispatches_to_workbench() {
        assert_eq!(
            classify_top_level_args(vec![OsString::from("/tmp/peregrine-project")]),
            TopLevelDispatch::Workbench(Some(PathBuf::from("/tmp/peregrine-project")))
        );
    }

    #[test]
    fn helper_args_bypass_mode_dispatch() {
        assert_eq!(
            classify_top_level_args(vec![OsString::from(
                helper_args::BYTECODE_VIEWER_HELPER_ARG
            )]),
            TopLevelDispatch::CliOrHelper(vec![OsString::from(
                helper_args::BYTECODE_VIEWER_HELPER_ARG
            )])
        );
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
    fn package_load_running_renders_spinner_in_inspector() {
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
    fn package_load_completion_renders_in_inspector_without_modal() {
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
    fn navigation_moves_between_workbench_sections() {
        let temp = tempfile::tempdir().expect("temp dir");
        let mut app = App::new(temp.path(), EditorMode::Standard).expect("app");

        assert_eq!(app.focus, FocusPane::Explorer);
        workbench_nav(&mut app, KeyCode::Char('l'));
        assert_eq!(app.focus, FocusPane::Editor);

        workbench_nav(&mut app, KeyCode::Char('j'));
        assert_eq!(app.focus, FocusPane::Input);

        workbench_nav(&mut app, KeyCode::Char('l'));
        assert_eq!(app.focus, FocusPane::Inspector);

        workbench_nav(&mut app, KeyCode::Char('h'));
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
    fn bytecode_tab_hides_inspector_until_other_tab_selected() {
        let temp = tempfile::tempdir().expect("temp dir");
        let mut app = App::new(temp.path(), EditorMode::Standard).expect("app");

        app.set_active_tab(WorkbenchTab::Bytecode);
        assert!(!app.inspector_visible());

        let backend = TestBackend::new(120, 30);
        let mut terminal = Terminal::new(backend).expect("terminal");
        terminal.draw(|frame| app.render(frame)).expect("draw");
        let rendered = buffer_to_string(terminal.backend().buffer());

        assert!(rendered.contains("Bytecode"));
        assert!(!rendered.contains("Inspector"));

        app.set_active_tab(WorkbenchTab::Cfg);
        assert!(app.inspector_visible());

        let backend = TestBackend::new(120, 30);
        let mut terminal = Terminal::new(backend).expect("terminal");
        terminal.draw(|frame| app.render(frame)).expect("draw");
        let rendered = buffer_to_string(terminal.backend().buffer());

        assert!(rendered.contains("cfg is not loaded"));
        assert!(rendered.contains("No package status yet"));
    }

    #[test]
    fn hidden_inspector_cannot_keep_focus_on_bytecode_tab() {
        let temp = tempfile::tempdir().expect("temp dir");
        let mut app = App::new(temp.path(), EditorMode::Standard).expect("app");

        app.focus = FocusPane::Inspector;
        app.set_active_tab(WorkbenchTab::Bytecode);
        assert_eq!(app.focus, FocusPane::Editor);

        app.apply_navigation_command(NavigationCommand::Focus(FocusPane::Inspector));
        assert_eq!(app.focus, FocusPane::Editor);

        app.set_active_tab(WorkbenchTab::Code);
        app.apply_navigation_command(NavigationCommand::Focus(FocusPane::Inspector));
        assert_eq!(app.focus, FocusPane::Inspector);
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
        assert_eq!(app.focus, FocusPane::Inspector);
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
    fn mouse_clicking_bytecode_tab_hides_inspector_without_panic() {
        let temp = tempfile::tempdir().expect("temp dir");
        let mut app = App::new(temp.path(), EditorMode::Standard).expect("app");
        let backend = TestBackend::new(120, 30);
        let mut terminal = Terminal::new(backend).expect("terminal");

        terminal.draw(|frame| app.render(frame)).expect("draw");
        let bytecode_tab = app.layout.tab_hit_areas[1].1;
        app.handle_mouse_event(left_click(bytecode_tab.x + 1, bytecode_tab.y + 1));

        assert_eq!(app.active_tab, WorkbenchTab::Bytecode);
        assert!(!app.inspector_visible());
        assert!(matches!(app.bytecode, BytecodePane::Empty));
        assert!(app.bytecode_loader_rx.is_none());
        terminal.draw(|frame| app.render(frame)).expect("draw");
        assert!(app.layout.inspector.is_none());
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

        let original = app.theme.name;
        workbench_nav(&mut app, KeyCode::Char(']'));
        assert_eq!(app.theme.name, original.next());
        assert!(app.status.contains("Theme:"));

        workbench_nav(&mut app, KeyCode::Char('['));
        assert_eq!(app.theme.name, original);
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
        assert!(rendered.contains("No package status yet"));
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
}
