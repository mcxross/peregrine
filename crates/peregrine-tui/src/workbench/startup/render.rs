use crate::workbench::prelude::*;

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};

impl App {
    pub(crate) fn render_startup(&self, frame: &mut Frame<'_>, area: Rect) {
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

    pub(crate) fn render_invalid_package_prompt(
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
            startup_option_line(
                self,
                prompt.selected == InvalidPackageAction::GoBack,
                "3",
                "Go back to the previous screen or exit",
            ),
            Line::from(""),
            Line::styled("Use Up/Down or j/k, then Enter.", self.muted_style()),
        ];
        self.render_startup_message(frame, area, "Move Package", lines);
    }

    pub(crate) fn render_package_name_prompt(
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

    pub(crate) fn render_trust_prompt(
        &self,
        frame: &mut Frame<'_>,
        area: Rect,
        prompt: &TrustPrompt,
    ) {
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

    pub(crate) fn render_startup_message(
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

    pub(crate) fn startup_label_line(&self, label: &'static str, value: String) -> Line<'static> {
        Line::from(vec![
            Span::styled(format!("{label}: "), self.muted_style()),
            Span::styled(value, self.base_style()),
        ])
    }
}
