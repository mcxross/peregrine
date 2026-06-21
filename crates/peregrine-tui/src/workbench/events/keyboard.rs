use crate::sui::package_loader::persist_trust_for_resolution;
use crate::theme::ThemeName;
use crate::workbench::prelude::*;

use crate::chat;
use crate::navigation::NavigationIntent;
use ratatui::crossterm::event::{
    KeyCode, KeyEvent, KeyModifiers,
};

impl App {
    pub fn handle_key_event(&mut self, key: KeyEvent) {
        if self.pending_close.is_some() {
            self.handle_close_confirmation_key(key);
            return;
        }
        if !self.startup.is_workbench() {
            self.handle_startup_key(key);
            return;
        }

        if self.active_tab == WorkbenchTab::Chat {
            self.handle_chat_key_event(key);
            return;
        }

        match self.navigation.translate(key, self.focus) {
            NavigationIntent::Command(command) => self.apply_navigation_command(command),
            NavigationIntent::PassThrough => match self.focus {
                FocusPane::Explorer => self.handle_explorer_key(key),
                FocusPane::Tabs => self.handle_tabs_key(key),
                FocusPane::FileTabs => self.handle_file_tabs_key(key),
                FocusPane::Editor => self.handle_editor_key(key),
                FocusPane::Input => self.handle_editor_key(key),
            },
        }
    }

    pub(crate) fn tick_chat(&mut self) {
        if self.active_tab != WorkbenchTab::Chat {
            return;
        }
        let action = self.chat.tick(&self.explorer.root);
        self.apply_chat_action(action);
    }

    pub(crate) fn handle_chat_key_event(&mut self, key: KeyEvent) {
        match self.navigation.translate_workbench_navigation_only(key) {
            NavigationIntent::Command(command) => self.apply_navigation_command(command),
            NavigationIntent::PassThrough => {
                let action = self.chat.handle_key(&self.explorer.root, key);
                self.apply_chat_action(action);
            }
        }
    }

    pub(crate) fn apply_chat_action(&mut self, action: chat::ChatAction) {
        match action {
            chat::ChatAction::None => {}
            chat::ChatAction::FocusCode => {
                self.focus_code_editor();
                self.status = "Returned to workbench".to_string();
            }
            chat::ChatAction::Quit => {
                self.exit = Some(WorkbenchExit::Quit);
            }
            chat::ChatAction::ThemeSelected(name) => match name.parse::<ThemeName>() {
                Ok(theme_name) => {
                    self.theme.set(theme_name);
                    self.sync_syntax_theme();
                    self.invalidate_workbench_views();
                    self.status = format!("Theme: {}", self.theme.current_name());
                }
                Err(err) => {
                    self.status = format!("Theme unavailable: {err}");
                }
            },
        }
    }

    pub(crate) fn handle_startup_key(&mut self, key: KeyEvent) {
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
                KeyCode::Up | KeyCode::Char('k') if plain => {
                    prompt.selected = prompt.selected.toggle_back();
                    self.startup = WorkbenchStartupState::InvalidPackageChoice(prompt);
                }
                KeyCode::Down | KeyCode::Char('j') if plain => {
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
                KeyCode::Char('3') | KeyCode::Esc if plain => {
                    prompt.selected = InvalidPackageAction::GoBack;
                    self.exit = Some(WorkbenchExit::SwitchToAgent);
                }
                KeyCode::Enter => match prompt.selected {
                    InvalidPackageAction::CreatePackage => self.open_package_name_prompt(prompt),
                    InvalidPackageAction::ProceedAnyway => self.apply_trust_resolution(
                        prompt.trust_resolution,
                        TrustPostAction::EnterWorkbench,
                    ),
                    InvalidPackageAction::GoBack => {
                        self.exit = Some(WorkbenchExit::SwitchToAgent);
                    }
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

    pub(crate) fn open_package_name_prompt(&mut self, prompt: InvalidPackagePrompt) {
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

    pub(crate) fn accept_trust_prompt(&mut self, mut prompt: TrustPrompt) {
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
}
