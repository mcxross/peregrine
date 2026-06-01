mod args;
pub mod helper_args;
mod keybinds;
mod navigation;
mod output;
pub mod sui;
pub mod tabs;
mod workflow;

use crate::navigation::{Navigation, NavigationCommand, NavigationIntent};
use crate::tabs::TabNav;
use clap::Parser;
use peregrine_config::CONFIG_TOML_FILE;
use peregrine_config::config_toml::ConfigToml;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::{Constraint, Direction, Layout, Position, Rect};
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};
use ratatui::{DefaultTerminal, Frame};
use std::cmp::Ordering;
use std::collections::BTreeSet;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

const UNDO_LIMIT: usize = 100;
const PAGE_SIZE: usize = 12;
const WORKBENCH_TAB_LABELS: [&str; 5] = ["code", "bytecode", "cfg", "call graph", "type graph"];

pub fn run() -> io::Result<i32> {
    run_from_env_args(std::env::args_os())
}

pub fn run_from_env_args<I>(args: I) -> io::Result<i32>
where
    I: IntoIterator<Item = OsString>,
{
    let mut args = args.into_iter();
    let _binary = args.next();

    match args.next() {
        Some(arg) => Ok(run_cli_or_helper_from_args(
            std::iter::once(arg).chain(args),
        )),
        None => {
            run_tui()?;
            Ok(0)
        }
    }
}

pub fn run_tui() -> io::Result<()> {
    let mut app = App::from_current_dir()?;
    let mut terminal = ratatui::try_init()?;
    let result = app.run(&mut terminal);
    ratatui::restore();
    result
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

pub struct App {
    mode: AppMode,
    focus: FocusPane,
    active_tab: WorkbenchTab,
    editor_mode: EditorMode,
    vim_state: VimState,
    navigation: Navigation,
    explorer: Explorer,
    editor: EditorBuffer,
    input: CommandInput,
    should_quit: bool,
    status: String,
}

impl App {
    pub fn from_current_dir() -> io::Result<Self> {
        let cwd = std::env::current_dir()?;
        Self::new(cwd, configured_editor_mode())
    }

    pub fn new(root: impl AsRef<Path>, editor_mode: EditorMode) -> io::Result<Self> {
        keybinds::init_default_keybindings()?;
        Ok(Self {
            mode: AppMode::default(),
            focus: FocusPane::Explorer,
            active_tab: WorkbenchTab::Code,
            editor_mode,
            vim_state: VimState::Normal,
            navigation: Navigation::default(),
            explorer: Explorer::new(root)?,
            editor: EditorBuffer::new_empty(),
            input: CommandInput::default(),
            should_quit: false,
            status: keybinds::default_hint(),
        })
    }

    fn run(&mut self, terminal: &mut DefaultTerminal) -> io::Result<()> {
        loop {
            terminal.draw(|frame| self.render(frame))?;
            if self.should_quit {
                return Ok(());
            }
            if !event::poll(Duration::from_millis(250))? {
                continue;
            }
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    self.handle_key_event(key);
                }
            }
        }
    }

    pub fn handle_key_event(&mut self, key: KeyEvent) {
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

    fn apply_navigation_command(&mut self, command: NavigationCommand) {
        match command {
            NavigationCommand::Quit => self.should_quit = true,
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
            NavigationCommand::Focus(pane) => self.focus = pane,
            NavigationCommand::FocusCodeEditor => self.focus_code_editor(),
            NavigationCommand::FocusNext => self.focus = navigation::next_focus(self.focus),
            NavigationCommand::FocusPrevious => {
                self.focus = navigation::previous_focus(self.focus);
            }
            NavigationCommand::MoveFocus(direction) => {
                self.focus = navigation::move_focus(self.focus, direction);
            }
            NavigationCommand::SelectTab(tab) => {
                self.active_tab = tab;
                self.focus = FocusPane::Tabs;
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
            KeyCode::Down | KeyCode::Enter | KeyCode::Esc => self.focus = FocusPane::Editor,
            _ => {}
        }
    }

    fn handle_editor_key(&mut self, key: KeyEvent) {
        if self.active_tab != WorkbenchTab::Code {
            return;
        }

        match self.editor_mode {
            EditorMode::Standard => self.editor.handle_standard_key(key),
            EditorMode::Vim => {
                if self.vim_state == VimState::Insert {
                    if key.code == KeyCode::Esc {
                        self.vim_state = VimState::Normal;
                    } else {
                        self.editor.handle_standard_key(key);
                    }
                } else {
                    self.handle_vim_normal_key(key);
                }
            }
        }
    }

    fn handle_vim_normal_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => self.vim_state = VimState::Normal,
            KeyCode::Char('h') => self.editor.move_left(),
            KeyCode::Char('j') => self.editor.move_down(),
            KeyCode::Char('k') => self.editor.move_up(),
            KeyCode::Char('l') => self.editor.move_right(),
            KeyCode::Char('i') => self.vim_state = VimState::Insert,
            KeyCode::Char('a') => {
                self.editor.move_right();
                self.vim_state = VimState::Insert;
            }
            KeyCode::Char('A') => {
                self.editor.move_line_end();
                self.vim_state = VimState::Insert;
            }
            KeyCode::Char('I') => {
                self.editor.move_line_start();
                self.vim_state = VimState::Insert;
            }
            KeyCode::Char('o') => {
                self.editor.open_line_below();
                self.vim_state = VimState::Insert;
            }
            KeyCode::Char('O') => {
                self.editor.open_line_above();
                self.vim_state = VimState::Insert;
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
            KeyCode::Esc => self.focus = FocusPane::Editor,
            _ => self.input.handle_key(key),
        }
    }

    fn handle_inspector_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Left | KeyCode::Char('h') | KeyCode::Esc => self.focus = FocusPane::Editor,
            _ => {}
        }
    }

    fn focus_code_editor(&mut self) {
        self.focus = FocusPane::Editor;
        self.active_tab = WorkbenchTab::Code;
    }

    fn next_tab(&mut self) {
        let index = self.active_tab.index();
        self.active_tab = WorkbenchTab::ALL[(index + 1) % WorkbenchTab::ALL.len()];
    }

    fn previous_tab(&mut self) {
        let index = self.active_tab.index();
        self.active_tab =
            WorkbenchTab::ALL[(index + WorkbenchTab::ALL.len() - 1) % WorkbenchTab::ALL.len()];
    }

    fn toggle_editor_mode(&mut self) {
        self.editor_mode = match self.editor_mode {
            EditorMode::Standard => EditorMode::Vim,
            EditorMode::Vim => EditorMode::Standard,
        };
        self.vim_state = VimState::Normal;
        self.status = format!("Editor mode: {}", self.editor_mode_label());
    }

    fn editor_mode_label(&self) -> &'static str {
        match self.editor_mode {
            EditorMode::Standard => "standard",
            EditorMode::Vim => match self.vim_state {
                VimState::Normal => "vim normal",
                VimState::Insert => "vim insert",
            },
        }
    }

    fn app_mode_label(&self) -> &'static str {
        match self.mode {
            AppMode::Workbench => "workbench",
            AppMode::Agent => "agent",
        }
    }

    fn open_file(&mut self, path: PathBuf) {
        if self.editor.dirty && self.editor.path.as_ref() != Some(&path) {
            self.status = String::from("Unsaved changes: Ctrl-S to save or Ctrl-R to reload first");
            return;
        }

        match self.editor.open_file(&path) {
            Ok(()) => {
                self.focus = FocusPane::Editor;
                self.active_tab = WorkbenchTab::Code;
                self.status = format!("Opened {}", path.display());
            }
            Err(error) => {
                self.status = format!("Could not open {}: {error}", path.display());
            }
        }
    }

    fn save_current_file(&mut self) {
        match self.editor.save() {
            Ok(()) => self.status = String::from("Saved"),
            Err(error) => self.status = format!("Save failed: {error}"),
        }
    }

    fn reload_current_file(&mut self) {
        match self.editor.reload() {
            Ok(()) => self.status = String::from("Reloaded"),
            Err(error) => self.status = format!("Reload failed: {error}"),
        }
    }

    pub fn render(&mut self, frame: &mut Frame<'_>) {
        let columns = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(25),
                Constraint::Percentage(50),
                Constraint::Percentage(25),
            ])
            .split(frame.area());

        self.render_explorer(frame, columns[0]);
        self.render_center(frame, columns[1]);
        self.render_inspector(frame, columns[2]);
    }

    fn render_explorer(&self, frame: &mut Frame<'_>, area: Rect) {
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
                ListItem::new(label)
            })
            .collect::<Vec<_>>();
        let block = Block::default()
            .borders(Borders::ALL)
            .title(focused_title("Explorer", self.focus == FocusPane::Explorer));
        let mut state = ListState::default().with_selected(Some(self.explorer.selected()));
        let list = List::new(items)
            .block(block)
            .highlight_style(Style::default().fg(Color::Black).bg(Color::White))
            .highlight_symbol("> ");
        frame.render_stateful_widget(list, area, &mut state);
    }

    fn render_center(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(3),
                Constraint::Length(3),
            ])
            .split(area);

        let border_style = if self.focus == FocusPane::Tabs {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        let tabs = TabNav::new(&WORKBENCH_TAB_LABELS, self.active_tab.index())
            .style(Style::default().fg(Color::Gray))
            .highlight_style(Style::default().fg(Color::Cyan))
            .border_style(border_style)
            .highlight_bold(true);
        frame.render_widget(tabs, rows[0]);

        match self.active_tab {
            WorkbenchTab::Code => self.render_editor(frame, rows[1]),
            tab => {
                let title = focused_title("View", self.focus == FocusPane::Editor);
                let paragraph = Paragraph::new(format!("{} view placeholder", tab.title()))
                    .block(Block::default().borders(Borders::ALL).title(title));
                frame.render_widget(paragraph, rows[1]);
            }
        }

        self.render_input(frame, rows[2]);
    }

    fn render_editor(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let inner_height = area.height.saturating_sub(2) as usize;
        self.editor.set_viewport_height(inner_height);
        let title = format!(
            "{}{} [{}]",
            self.editor.display_name(),
            if self.editor.dirty { " *" } else { "" },
            self.editor_mode_label()
        );
        let text = self.editor.text();
        let paragraph = Paragraph::new(text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(focused_title(&title, self.focus == FocusPane::Editor)),
            )
            .scroll((self.editor.scroll as u16, 0));
        frame.render_widget(paragraph, area);

        if self.focus == FocusPane::Editor && self.active_tab == WorkbenchTab::Code {
            let row = self.editor.cursor.row.saturating_sub(self.editor.scroll);
            if row < inner_height {
                let max_x = area.width.saturating_sub(2);
                let max_y = area.height.saturating_sub(2);
                let x = (self.editor.cursor.col as u16).min(max_x);
                let y = (row as u16).min(max_y);
                frame.set_cursor_position(Position::new(area.x + 1 + x, area.y + 1 + y));
            }
        }
    }

    fn render_input(&self, frame: &mut Frame<'_>, area: Rect) {
        let title = format!("Input - {}", self.status);
        let paragraph = Paragraph::new(self.input.text.as_str()).block(
            Block::default()
                .borders(Borders::ALL)
                .title(focused_title(&title, self.focus == FocusPane::Input)),
        );
        frame.render_widget(paragraph, area);

        if self.focus == FocusPane::Input {
            let max_x = area.width.saturating_sub(2);
            let x = (self.input.cursor as u16).min(max_x);
            frame.set_cursor_position(Position::new(area.x + 1 + x, area.y + 1));
        }
    }

    fn render_inspector(&self, frame: &mut Frame<'_>, area: Rect) {
        let selected_path = self
            .explorer
            .selected_path()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| String::from("<none>"));
        let edited_path = self
            .editor
            .path
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| String::from("<none>"));
        let dirty = if self.editor.dirty { "yes" } else { "no" };
        let text = [
            String::from("Inspector placeholder"),
            format!("selected: {selected_path}"),
            format!("file: {edited_path}"),
            format!("app mode: {}", self.app_mode_label()),
            format!("tab: {}", self.active_tab.title()),
            format!("dirty: {dirty}"),
            format!("mode: {}", self.editor_mode_label()),
        ]
        .join("\n");
        let paragraph = Paragraph::new(text).block(Block::default().borders(Borders::ALL).title(
            focused_title("Inspector", self.focus == FocusPane::Inspector),
        ));
        frame.render_widget(paragraph, area);
    }
}

fn focused_title(title: &str, focused: bool) -> String {
    if focused {
        format!("* {title}")
    } else {
        title.to_string()
    }
}

pub fn configured_editor_mode() -> EditorMode {
    match peregrine_utils_home_dir::find_peregrine_home() {
        Ok(home) => load_editor_mode_from_home(home.as_path()),
        Err(_) => EditorMode::Standard,
    }
}

pub fn load_editor_mode_from_home(home: &Path) -> EditorMode {
    let config_path = home.join(CONFIG_TOML_FILE);
    let Ok(contents) = fs::read_to_string(config_path) else {
        return EditorMode::Standard;
    };
    let Ok(config) = toml::from_str::<ConfigToml>(&contents) else {
        return EditorMode::Standard;
    };
    if config.tui.is_some_and(|tui| tui.vim_mode_default) {
        EditorMode::Vim
    } else {
        EditorMode::Standard
    }
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
    dirty: bool,
    undo_stack: Vec<EditorSnapshot>,
    yank: Vec<String>,
    pending_vim: Option<PendingVimCommand>,
    viewport_height: usize,
}

impl EditorBuffer {
    pub fn new_empty() -> Self {
        Self {
            path: None,
            lines: vec![String::new()],
            cursor: Cursor { row: 0, col: 0 },
            scroll: 0,
            dirty: false,
            undo_stack: Vec::new(),
            yank: Vec::new(),
            pending_vim: None,
            viewport_height: 1,
        }
    }

    pub fn open_file(&mut self, path: &Path) -> io::Result<()> {
        let contents = fs::read_to_string(path)?;
        self.path = Some(path.to_path_buf());
        self.lines = split_lines(&contents);
        self.cursor = Cursor { row: 0, col: 0 };
        self.scroll = 0;
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

    fn display_name(&self) -> String {
        self.path
            .as_ref()
            .and_then(|path| path.file_name())
            .and_then(|name| name.to_str())
            .map(str::to_string)
            .unwrap_or_else(|| String::from("No file"))
    }

    fn set_viewport_height(&mut self, height: usize) {
        self.viewport_height = height.max(1);
        self.ensure_cursor_visible();
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
    }

    fn move_line_end(&mut self) {
        self.cursor.col = char_len(&self.lines[self.cursor.row]);
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

#[derive(Default)]
pub struct CommandInput {
    text: String,
    cursor: usize,
}

impl CommandInput {
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::crossterm::event::KeyModifiers;

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
    fn navigation_moves_between_workbench_sections() {
        let temp = tempfile::tempdir().expect("temp dir");
        let mut app = App::new(temp.path(), EditorMode::Standard).expect("app");

        assert_eq!(app.focus, FocusPane::Explorer);
        workbench_nav(&mut app, KeyCode::Char('l'));
        assert_eq!(app.focus, FocusPane::Tabs);

        workbench_nav(&mut app, KeyCode::Char('j'));
        assert_eq!(app.focus, FocusPane::Editor);

        workbench_nav(&mut app, KeyCode::Char('j'));
        assert_eq!(app.focus, FocusPane::Input);

        workbench_nav(&mut app, KeyCode::Char('l'));
        assert_eq!(app.focus, FocusPane::Inspector);

        workbench_nav(&mut app, KeyCode::Char('h'));
        assert_eq!(app.focus, FocusPane::Editor);
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
    fn editor_tab_key_still_inserts_tab_character() {
        let temp = tempfile::tempdir().expect("temp dir");
        let mut app = App::new(temp.path(), EditorMode::Standard).expect("app");

        workbench_nav(&mut app, KeyCode::Char('c'));
        app.handle_key_event(key(KeyCode::Tab));

        assert_eq!(app.focus, FocusPane::Editor);
        assert_eq!(app.editor.text(), "\t");
    }

    #[test]
    fn workbench_prefix_selects_tabs_and_toggles_mode() {
        let temp = tempfile::tempdir().expect("temp dir");
        let mut app = App::new(temp.path(), EditorMode::Standard).expect("app");

        workbench_nav(&mut app, KeyCode::Char('3'));
        assert_eq!(app.focus, FocusPane::Tabs);
        assert_eq!(app.active_tab, WorkbenchTab::Cfg);

        workbench_nav(&mut app, KeyCode::Char('m'));
        assert_eq!(app.editor_mode, EditorMode::Vim);
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
    fn global_quit_shortcuts_set_should_quit() {
        let temp = tempfile::tempdir().expect("temp dir");
        let mut app = App::new(temp.path(), EditorMode::Standard).expect("app");

        app.handle_key_event(key_with_modifiers(
            KeyCode::Char('c'),
            KeyModifiers::CONTROL,
        ));
        assert!(app.should_quit);

        app.should_quit = false;
        app.handle_key_event(key_with_modifiers(
            KeyCode::Char('q'),
            KeyModifiers::CONTROL,
        ));
        assert!(app.should_quit);
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
        assert!(rendered.contains("Inspector placeholder"));
        assert!(rendered.contains("standard"));
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn key_with_modifiers(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, modifiers)
    }

    fn workbench_nav(app: &mut App, code: KeyCode) {
        app.handle_key_event(key_with_modifiers(
            KeyCode::Char('w'),
            KeyModifiers::CONTROL,
        ));
        app.handle_key_event(key(code));
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
