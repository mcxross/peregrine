//! Theme definitions and configuration.

use ratatui::style::Color;
use serde::{Deserialize, Serialize};

use crate::theme::palette::{GraphPalette, SyntaxPalette, ThemePalette};

/// Enumeration of all available Peregrine color themes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum ThemeName {
    /// Peregrine Night — default dark security workspace.
    #[default]
    PeregrineNight,
    /// Falcon Watch — command-center blue for scans, agents, and live monitoring.
    FalconWatch,
    /// Audit Dark — restrained dark mode for review, triage, and evidence work.
    AuditDark,
    /// Threat Matrix — high-contrast theme for suspicious paths and vulnerability analysis.
    ThreatMatrix,
    /// Bytecode Ember — warm theme for bytecode, CFGs, gas, and stack traces.
    BytecodeEmber,
    /// Sandbox Graphite — neutral theme for sandboxed execution and approvals.
    SandboxGraphite,
    /// Invariant Pine — calm green theme for specs, invariants, and formal reasoning.
    InvariantPine,
    /// Zero Day — bold neon security theme for exploit simulation and demos.
    ZeroDay,
    /// Evidence Light — clean light mode for findings, evidence packs, and reports.
    EvidenceLight,
    /// Whitebox — minimal light analysis theme for code review and diagrams.
    Whitebox,
}

impl ThemeName {
    /// Returns a slice containing all available theme names.
    #[must_use]
    pub const fn all() -> &'static [Self] {
        &[
            Self::PeregrineNight,
            Self::FalconWatch,
            Self::AuditDark,
            Self::ThreatMatrix,
            Self::BytecodeEmber,
            Self::SandboxGraphite,
            Self::InvariantPine,
            Self::ZeroDay,
            Self::EvidenceLight,
            Self::Whitebox,
        ]
    }

    /// Returns the human-readable display name for the theme.
    #[must_use]
    pub const fn display_name(self) -> &'static str {
        match self {
            Self::PeregrineNight => "Peregrine Night",
            Self::FalconWatch => "Falcon Watch",
            Self::AuditDark => "Audit Dark",
            Self::ThreatMatrix => "Threat Matrix",
            Self::BytecodeEmber => "Bytecode Ember",
            Self::SandboxGraphite => "Sandbox Graphite",
            Self::InvariantPine => "Invariant Pine",
            Self::ZeroDay => "Zero Day",
            Self::EvidenceLight => "Evidence Light",
            Self::Whitebox => "Whitebox",
        }
    }

    /// Returns the kebab-case slug for the theme.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::PeregrineNight => "peregrine-night",
            Self::FalconWatch => "falcon-watch",
            Self::AuditDark => "audit-dark",
            Self::ThreatMatrix => "threat-matrix",
            Self::BytecodeEmber => "bytecode-ember",
            Self::SandboxGraphite => "sandbox-graphite",
            Self::InvariantPine => "invariant-pine",
            Self::ZeroDay => "zero-day",
            Self::EvidenceLight => "evidence-light",
            Self::Whitebox => "whitebox",
        }
    }

    /// Returns the next theme in the list, wrapping around at the end.
    #[must_use]
    pub fn next(self) -> Self {
        let themes = Self::all();
        let current = themes.iter().position(|&theme| theme == self).unwrap_or(0);
        themes[(current + 1) % themes.len()]
    }

    /// Returns the previous theme in the list, wrapping around at the beginning.
    #[must_use]
    pub fn prev(self) -> Self {
        let themes = Self::all();
        let current = themes.iter().position(|&theme| theme == self).unwrap_or(0);
        themes[(current + themes.len() - 1) % themes.len()]
    }

    /// Returns the color palette for this theme.
    #[must_use]
    #[allow(clippy::too_many_lines)]
    pub const fn palette(self) -> ThemePalette {
        match self {
            Self::PeregrineNight => ThemePalette {
                accent: Color::Rgb(72, 191, 227),
                secondary: Color::Rgb(124, 92, 255),
                bg: Color::Rgb(10, 14, 22),
                fg: Color::Rgb(226, 232, 240),
                muted: Color::Rgb(100, 116, 139),
                selection: Color::Rgb(24, 33, 47),
                error: Color::Rgb(248, 81, 73),
                warning: Color::Rgb(245, 158, 11),
                success: Color::Rgb(34, 197, 94),
                info: Color::Rgb(56, 189, 248),
                syntax: SyntaxPalette {
                    text: Color::Rgb(226, 232, 240),
                    gutter: Color::Rgb(71, 85, 105),
                    active_line: Color::Rgb(16, 23, 35),
                    comment: Color::Rgb(100, 116, 139),
                    keyword: Color::Rgb(124, 92, 255),
                    type_name: Color::Rgb(94, 234, 212),
                    function: Color::Rgb(125, 211, 252),
                    variable: Color::Rgb(226, 232, 240),
                    string: Color::Rgb(134, 239, 172),
                    number: Color::Rgb(253, 186, 116),
                    operator: Color::Rgb(148, 163, 184),
                    macro_name: Color::Rgb(216, 180, 254),
                    diff_add: Color::Rgb(34, 197, 94),
                    diff_delete: Color::Rgb(248, 81, 73),
                    diff_change: Color::Rgb(245, 158, 11),
                    security_sensitive: Color::Rgb(251, 113, 133),
                },
                graph: GraphPalette {
                    node: Color::Rgb(72, 191, 227),
                    node_secondary: Color::Rgb(100, 116, 139),
                    selected_node: Color::Rgb(124, 92, 255),
                    entry_node: Color::Rgb(34, 197, 94),
                    exit_node: Color::Rgb(148, 163, 184),
                    critical_node: Color::Rgb(248, 81, 73),
                    warning_node: Color::Rgb(245, 158, 11),
                    safe_node: Color::Rgb(45, 212, 191),
                    edge: Color::Rgb(71, 85, 105),
                    data_flow: Color::Rgb(56, 189, 248),
                    control_flow: Color::Rgb(124, 92, 255),
                    call_edge: Color::Rgb(125, 211, 252),
                    ownership_edge: Color::Rgb(34, 197, 94),
                    attack_edge: Color::Rgb(248, 81, 73),
                    cluster: Color::Rgb(30, 41, 59),
                },
            },
            Self::FalconWatch => ThemePalette {
                accent: Color::Rgb(59, 130, 246),
                secondary: Color::Rgb(14, 165, 233),
                bg: Color::Rgb(8, 13, 24),
                fg: Color::Rgb(219, 234, 254),
                muted: Color::Rgb(96, 115, 139),
                selection: Color::Rgb(20, 34, 58),
                error: Color::Rgb(239, 68, 68),
                warning: Color::Rgb(234, 179, 8),
                success: Color::Rgb(16, 185, 129),
                info: Color::Rgb(125, 211, 252),
                syntax: SyntaxPalette {
                    text: Color::Rgb(219, 234, 254),
                    gutter: Color::Rgb(75, 95, 125),
                    active_line: Color::Rgb(13, 25, 46),
                    comment: Color::Rgb(96, 115, 139),
                    keyword: Color::Rgb(96, 165, 250),
                    type_name: Color::Rgb(103, 232, 249),
                    function: Color::Rgb(147, 197, 253),
                    variable: Color::Rgb(219, 234, 254),
                    string: Color::Rgb(110, 231, 183),
                    number: Color::Rgb(253, 224, 71),
                    operator: Color::Rgb(148, 163, 184),
                    macro_name: Color::Rgb(196, 181, 253),
                    diff_add: Color::Rgb(16, 185, 129),
                    diff_delete: Color::Rgb(239, 68, 68),
                    diff_change: Color::Rgb(234, 179, 8),
                    security_sensitive: Color::Rgb(251, 113, 133),
                },
                graph: GraphPalette {
                    node: Color::Rgb(59, 130, 246),
                    node_secondary: Color::Rgb(96, 115, 139),
                    selected_node: Color::Rgb(14, 165, 233),
                    entry_node: Color::Rgb(16, 185, 129),
                    exit_node: Color::Rgb(148, 163, 184),
                    critical_node: Color::Rgb(239, 68, 68),
                    warning_node: Color::Rgb(234, 179, 8),
                    safe_node: Color::Rgb(45, 212, 191),
                    edge: Color::Rgb(51, 65, 85),
                    data_flow: Color::Rgb(125, 211, 252),
                    control_flow: Color::Rgb(96, 165, 250),
                    call_edge: Color::Rgb(147, 197, 253),
                    ownership_edge: Color::Rgb(16, 185, 129),
                    attack_edge: Color::Rgb(239, 68, 68),
                    cluster: Color::Rgb(20, 34, 58),
                },
            },
            Self::AuditDark => ThemePalette {
                accent: Color::Rgb(148, 163, 184),
                secondary: Color::Rgb(99, 102, 241),
                bg: Color::Rgb(15, 17, 21),
                fg: Color::Rgb(229, 231, 235),
                muted: Color::Rgb(107, 114, 128),
                selection: Color::Rgb(31, 36, 46),
                error: Color::Rgb(220, 38, 38),
                warning: Color::Rgb(217, 119, 6),
                success: Color::Rgb(22, 163, 74),
                info: Color::Rgb(37, 99, 235),
                syntax: SyntaxPalette {
                    text: Color::Rgb(229, 231, 235),
                    gutter: Color::Rgb(75, 85, 99),
                    active_line: Color::Rgb(23, 27, 34),
                    comment: Color::Rgb(107, 114, 128),
                    keyword: Color::Rgb(129, 140, 248),
                    type_name: Color::Rgb(45, 212, 191),
                    function: Color::Rgb(96, 165, 250),
                    variable: Color::Rgb(229, 231, 235),
                    string: Color::Rgb(134, 239, 172),
                    number: Color::Rgb(251, 191, 36),
                    operator: Color::Rgb(156, 163, 175),
                    macro_name: Color::Rgb(192, 132, 252),
                    diff_add: Color::Rgb(22, 163, 74),
                    diff_delete: Color::Rgb(220, 38, 38),
                    diff_change: Color::Rgb(217, 119, 6),
                    security_sensitive: Color::Rgb(244, 63, 94),
                },
                graph: GraphPalette {
                    node: Color::Rgb(148, 163, 184),
                    node_secondary: Color::Rgb(107, 114, 128),
                    selected_node: Color::Rgb(99, 102, 241),
                    entry_node: Color::Rgb(22, 163, 74),
                    exit_node: Color::Rgb(156, 163, 175),
                    critical_node: Color::Rgb(220, 38, 38),
                    warning_node: Color::Rgb(217, 119, 6),
                    safe_node: Color::Rgb(22, 163, 74),
                    edge: Color::Rgb(75, 85, 99),
                    data_flow: Color::Rgb(37, 99, 235),
                    control_flow: Color::Rgb(99, 102, 241),
                    call_edge: Color::Rgb(96, 165, 250),
                    ownership_edge: Color::Rgb(22, 163, 74),
                    attack_edge: Color::Rgb(220, 38, 38),
                    cluster: Color::Rgb(31, 36, 46),
                },
            },
            Self::ThreatMatrix => ThemePalette {
                accent: Color::Rgb(6, 182, 212),
                secondary: Color::Rgb(244, 63, 94),
                bg: Color::Rgb(9, 9, 15),
                fg: Color::Rgb(241, 245, 249),
                muted: Color::Rgb(100, 116, 139),
                selection: Color::Rgb(30, 41, 59),
                error: Color::Rgb(255, 49, 77),
                warning: Color::Rgb(251, 191, 36),
                success: Color::Rgb(45, 212, 191),
                info: Color::Rgb(56, 189, 248),
                syntax: SyntaxPalette {
                    text: Color::Rgb(241, 245, 249),
                    gutter: Color::Rgb(82, 96, 119),
                    active_line: Color::Rgb(18, 24, 38),
                    comment: Color::Rgb(100, 116, 139),
                    keyword: Color::Rgb(244, 63, 94),
                    type_name: Color::Rgb(34, 211, 238),
                    function: Color::Rgb(56, 189, 248),
                    variable: Color::Rgb(241, 245, 249),
                    string: Color::Rgb(45, 212, 191),
                    number: Color::Rgb(251, 191, 36),
                    operator: Color::Rgb(203, 213, 225),
                    macro_name: Color::Rgb(251, 113, 133),
                    diff_add: Color::Rgb(45, 212, 191),
                    diff_delete: Color::Rgb(255, 49, 77),
                    diff_change: Color::Rgb(251, 191, 36),
                    security_sensitive: Color::Rgb(255, 49, 77),
                },
                graph: GraphPalette {
                    node: Color::Rgb(6, 182, 212),
                    node_secondary: Color::Rgb(100, 116, 139),
                    selected_node: Color::Rgb(244, 63, 94),
                    entry_node: Color::Rgb(45, 212, 191),
                    exit_node: Color::Rgb(148, 163, 184),
                    critical_node: Color::Rgb(255, 49, 77),
                    warning_node: Color::Rgb(251, 191, 36),
                    safe_node: Color::Rgb(45, 212, 191),
                    edge: Color::Rgb(71, 85, 105),
                    data_flow: Color::Rgb(6, 182, 212),
                    control_flow: Color::Rgb(56, 189, 248),
                    call_edge: Color::Rgb(34, 211, 238),
                    ownership_edge: Color::Rgb(45, 212, 191),
                    attack_edge: Color::Rgb(255, 49, 77),
                    cluster: Color::Rgb(30, 41, 59),
                },
            },
            Self::BytecodeEmber => ThemePalette {
                accent: Color::Rgb(251, 146, 60),
                secondary: Color::Rgb(250, 204, 21),
                bg: Color::Rgb(18, 13, 10),
                fg: Color::Rgb(254, 243, 199),
                muted: Color::Rgb(146, 111, 77),
                selection: Color::Rgb(45, 32, 24),
                error: Color::Rgb(239, 68, 68),
                warning: Color::Rgb(245, 158, 11),
                success: Color::Rgb(132, 204, 22),
                info: Color::Rgb(14, 165, 233),
                syntax: SyntaxPalette {
                    text: Color::Rgb(254, 243, 199),
                    gutter: Color::Rgb(120, 89, 60),
                    active_line: Color::Rgb(32, 23, 17),
                    comment: Color::Rgb(146, 111, 77),
                    keyword: Color::Rgb(251, 146, 60),
                    type_name: Color::Rgb(252, 211, 77),
                    function: Color::Rgb(253, 186, 116),
                    variable: Color::Rgb(254, 243, 199),
                    string: Color::Rgb(190, 242, 100),
                    number: Color::Rgb(250, 204, 21),
                    operator: Color::Rgb(214, 183, 132),
                    macro_name: Color::Rgb(248, 113, 113),
                    diff_add: Color::Rgb(132, 204, 22),
                    diff_delete: Color::Rgb(239, 68, 68),
                    diff_change: Color::Rgb(245, 158, 11),
                    security_sensitive: Color::Rgb(248, 113, 113),
                },
                graph: GraphPalette {
                    node: Color::Rgb(251, 146, 60),
                    node_secondary: Color::Rgb(146, 111, 77),
                    selected_node: Color::Rgb(250, 204, 21),
                    entry_node: Color::Rgb(132, 204, 22),
                    exit_node: Color::Rgb(214, 183, 132),
                    critical_node: Color::Rgb(239, 68, 68),
                    warning_node: Color::Rgb(245, 158, 11),
                    safe_node: Color::Rgb(132, 204, 22),
                    edge: Color::Rgb(120, 89, 60),
                    data_flow: Color::Rgb(14, 165, 233),
                    control_flow: Color::Rgb(251, 146, 60),
                    call_edge: Color::Rgb(253, 186, 116),
                    ownership_edge: Color::Rgb(132, 204, 22),
                    attack_edge: Color::Rgb(239, 68, 68),
                    cluster: Color::Rgb(45, 32, 24),
                },
            },
            Self::SandboxGraphite => ThemePalette {
                accent: Color::Rgb(34, 211, 238),
                secondary: Color::Rgb(163, 163, 163),
                bg: Color::Rgb(18, 18, 20),
                fg: Color::Rgb(229, 229, 229),
                muted: Color::Rgb(115, 115, 115),
                selection: Color::Rgb(38, 38, 42),
                error: Color::Rgb(248, 113, 113),
                warning: Color::Rgb(250, 204, 21),
                success: Color::Rgb(74, 222, 128),
                info: Color::Rgb(96, 165, 250),
                syntax: SyntaxPalette {
                    text: Color::Rgb(229, 229, 229),
                    gutter: Color::Rgb(100, 100, 100),
                    active_line: Color::Rgb(28, 28, 32),
                    comment: Color::Rgb(115, 115, 115),
                    keyword: Color::Rgb(34, 211, 238),
                    type_name: Color::Rgb(147, 197, 253),
                    function: Color::Rgb(103, 232, 249),
                    variable: Color::Rgb(229, 229, 229),
                    string: Color::Rgb(134, 239, 172),
                    number: Color::Rgb(250, 204, 21),
                    operator: Color::Rgb(180, 180, 180),
                    macro_name: Color::Rgb(216, 180, 254),
                    diff_add: Color::Rgb(74, 222, 128),
                    diff_delete: Color::Rgb(248, 113, 113),
                    diff_change: Color::Rgb(250, 204, 21),
                    security_sensitive: Color::Rgb(251, 113, 133),
                },
                graph: GraphPalette {
                    node: Color::Rgb(34, 211, 238),
                    node_secondary: Color::Rgb(115, 115, 115),
                    selected_node: Color::Rgb(163, 163, 163),
                    entry_node: Color::Rgb(74, 222, 128),
                    exit_node: Color::Rgb(180, 180, 180),
                    critical_node: Color::Rgb(248, 113, 113),
                    warning_node: Color::Rgb(250, 204, 21),
                    safe_node: Color::Rgb(74, 222, 128),
                    edge: Color::Rgb(82, 82, 91),
                    data_flow: Color::Rgb(96, 165, 250),
                    control_flow: Color::Rgb(34, 211, 238),
                    call_edge: Color::Rgb(103, 232, 249),
                    ownership_edge: Color::Rgb(74, 222, 128),
                    attack_edge: Color::Rgb(248, 113, 113),
                    cluster: Color::Rgb(38, 38, 42),
                },
            },
            Self::InvariantPine => ThemePalette {
                accent: Color::Rgb(52, 211, 153),
                secondary: Color::Rgb(132, 204, 22),
                bg: Color::Rgb(10, 20, 16),
                fg: Color::Rgb(220, 252, 231),
                muted: Color::Rgb(94, 119, 101),
                selection: Color::Rgb(21, 38, 31),
                error: Color::Rgb(248, 113, 113),
                warning: Color::Rgb(234, 179, 8),
                success: Color::Rgb(34, 197, 94),
                info: Color::Rgb(45, 212, 191),
                syntax: SyntaxPalette {
                    text: Color::Rgb(220, 252, 231),
                    gutter: Color::Rgb(82, 105, 90),
                    active_line: Color::Rgb(16, 30, 24),
                    comment: Color::Rgb(94, 119, 101),
                    keyword: Color::Rgb(52, 211, 153),
                    type_name: Color::Rgb(190, 242, 100),
                    function: Color::Rgb(110, 231, 183),
                    variable: Color::Rgb(220, 252, 231),
                    string: Color::Rgb(134, 239, 172),
                    number: Color::Rgb(253, 224, 71),
                    operator: Color::Rgb(187, 247, 208),
                    macro_name: Color::Rgb(167, 243, 208),
                    diff_add: Color::Rgb(34, 197, 94),
                    diff_delete: Color::Rgb(248, 113, 113),
                    diff_change: Color::Rgb(234, 179, 8),
                    security_sensitive: Color::Rgb(251, 113, 133),
                },
                graph: GraphPalette {
                    node: Color::Rgb(52, 211, 153),
                    node_secondary: Color::Rgb(94, 119, 101),
                    selected_node: Color::Rgb(132, 204, 22),
                    entry_node: Color::Rgb(34, 197, 94),
                    exit_node: Color::Rgb(187, 247, 208),
                    critical_node: Color::Rgb(248, 113, 113),
                    warning_node: Color::Rgb(234, 179, 8),
                    safe_node: Color::Rgb(34, 197, 94),
                    edge: Color::Rgb(62, 85, 72),
                    data_flow: Color::Rgb(45, 212, 191),
                    control_flow: Color::Rgb(52, 211, 153),
                    call_edge: Color::Rgb(110, 231, 183),
                    ownership_edge: Color::Rgb(34, 197, 94),
                    attack_edge: Color::Rgb(248, 113, 113),
                    cluster: Color::Rgb(21, 38, 31),
                },
            },
            Self::ZeroDay => ThemePalette {
                accent: Color::Rgb(0, 245, 212),
                secondary: Color::Rgb(255, 0, 110),
                bg: Color::Rgb(7, 5, 15),
                fg: Color::Rgb(248, 250, 252),
                muted: Color::Rgb(113, 113, 122),
                selection: Color::Rgb(39, 21, 58),
                error: Color::Rgb(255, 0, 84),
                warning: Color::Rgb(255, 190, 11),
                success: Color::Rgb(0, 255, 136),
                info: Color::Rgb(0, 180, 216),
                syntax: SyntaxPalette {
                    text: Color::Rgb(248, 250, 252),
                    gutter: Color::Rgb(113, 113, 122),
                    active_line: Color::Rgb(22, 15, 35),
                    comment: Color::Rgb(113, 113, 122),
                    keyword: Color::Rgb(255, 0, 110),
                    type_name: Color::Rgb(0, 245, 212),
                    function: Color::Rgb(0, 180, 216),
                    variable: Color::Rgb(248, 250, 252),
                    string: Color::Rgb(0, 255, 136),
                    number: Color::Rgb(255, 190, 11),
                    operator: Color::Rgb(255, 255, 255),
                    macro_name: Color::Rgb(255, 91, 173),
                    diff_add: Color::Rgb(0, 255, 136),
                    diff_delete: Color::Rgb(255, 0, 84),
                    diff_change: Color::Rgb(255, 190, 11),
                    security_sensitive: Color::Rgb(255, 0, 84),
                },
                graph: GraphPalette {
                    node: Color::Rgb(0, 245, 212),
                    node_secondary: Color::Rgb(113, 113, 122),
                    selected_node: Color::Rgb(255, 0, 110),
                    entry_node: Color::Rgb(0, 255, 136),
                    exit_node: Color::Rgb(248, 250, 252),
                    critical_node: Color::Rgb(255, 0, 84),
                    warning_node: Color::Rgb(255, 190, 11),
                    safe_node: Color::Rgb(0, 255, 136),
                    edge: Color::Rgb(96, 78, 126),
                    data_flow: Color::Rgb(0, 180, 216),
                    control_flow: Color::Rgb(0, 245, 212),
                    call_edge: Color::Rgb(137, 247, 254),
                    ownership_edge: Color::Rgb(0, 255, 136),
                    attack_edge: Color::Rgb(255, 0, 84),
                    cluster: Color::Rgb(39, 21, 58),
                },
            },
            Self::EvidenceLight => ThemePalette {
                accent: Color::Rgb(37, 99, 235),
                secondary: Color::Rgb(79, 70, 229),
                bg: Color::Rgb(248, 250, 252),
                fg: Color::Rgb(15, 23, 42),
                muted: Color::Rgb(100, 116, 139),
                selection: Color::Rgb(226, 232, 240),
                error: Color::Rgb(220, 38, 38),
                warning: Color::Rgb(202, 138, 4),
                success: Color::Rgb(22, 163, 74),
                info: Color::Rgb(2, 132, 199),
                syntax: SyntaxPalette {
                    text: Color::Rgb(15, 23, 42),
                    gutter: Color::Rgb(148, 163, 184),
                    active_line: Color::Rgb(241, 245, 249),
                    comment: Color::Rgb(100, 116, 139),
                    keyword: Color::Rgb(79, 70, 229),
                    type_name: Color::Rgb(14, 116, 144),
                    function: Color::Rgb(37, 99, 235),
                    variable: Color::Rgb(15, 23, 42),
                    string: Color::Rgb(22, 101, 52),
                    number: Color::Rgb(180, 83, 9),
                    operator: Color::Rgb(71, 85, 105),
                    macro_name: Color::Rgb(126, 34, 206),
                    diff_add: Color::Rgb(22, 163, 74),
                    diff_delete: Color::Rgb(220, 38, 38),
                    diff_change: Color::Rgb(202, 138, 4),
                    security_sensitive: Color::Rgb(190, 18, 60),
                },
                graph: GraphPalette {
                    node: Color::Rgb(37, 99, 235),
                    node_secondary: Color::Rgb(100, 116, 139),
                    selected_node: Color::Rgb(79, 70, 229),
                    entry_node: Color::Rgb(22, 163, 74),
                    exit_node: Color::Rgb(71, 85, 105),
                    critical_node: Color::Rgb(220, 38, 38),
                    warning_node: Color::Rgb(202, 138, 4),
                    safe_node: Color::Rgb(22, 163, 74),
                    edge: Color::Rgb(148, 163, 184),
                    data_flow: Color::Rgb(2, 132, 199),
                    control_flow: Color::Rgb(79, 70, 229),
                    call_edge: Color::Rgb(37, 99, 235),
                    ownership_edge: Color::Rgb(22, 163, 74),
                    attack_edge: Color::Rgb(220, 38, 38),
                    cluster: Color::Rgb(226, 232, 240),
                },
            },
            Self::Whitebox => ThemePalette {
                accent: Color::Rgb(14, 116, 144),
                secondary: Color::Rgb(88, 80, 236),
                bg: Color::Rgb(244, 247, 250),
                fg: Color::Rgb(17, 24, 39),
                muted: Color::Rgb(107, 114, 128),
                selection: Color::Rgb(213, 219, 229),
                error: Color::Rgb(185, 28, 28),
                warning: Color::Rgb(180, 83, 9),
                success: Color::Rgb(21, 128, 61),
                info: Color::Rgb(3, 105, 161),
                syntax: SyntaxPalette {
                    text: Color::Rgb(17, 24, 39),
                    gutter: Color::Rgb(156, 163, 175),
                    active_line: Color::Rgb(235, 240, 245),
                    comment: Color::Rgb(107, 114, 128),
                    keyword: Color::Rgb(88, 80, 236),
                    type_name: Color::Rgb(14, 116, 144),
                    function: Color::Rgb(3, 105, 161),
                    variable: Color::Rgb(17, 24, 39),
                    string: Color::Rgb(21, 128, 61),
                    number: Color::Rgb(180, 83, 9),
                    operator: Color::Rgb(75, 85, 99),
                    macro_name: Color::Rgb(126, 34, 206),
                    diff_add: Color::Rgb(21, 128, 61),
                    diff_delete: Color::Rgb(185, 28, 28),
                    diff_change: Color::Rgb(180, 83, 9),
                    security_sensitive: Color::Rgb(190, 18, 60),
                },
                graph: GraphPalette {
                    node: Color::Rgb(14, 116, 144),
                    node_secondary: Color::Rgb(107, 114, 128),
                    selected_node: Color::Rgb(88, 80, 236),
                    entry_node: Color::Rgb(21, 128, 61),
                    exit_node: Color::Rgb(75, 85, 99),
                    critical_node: Color::Rgb(185, 28, 28),
                    warning_node: Color::Rgb(180, 83, 9),
                    safe_node: Color::Rgb(21, 128, 61),
                    edge: Color::Rgb(156, 163, 175),
                    data_flow: Color::Rgb(3, 105, 161),
                    control_flow: Color::Rgb(88, 80, 236),
                    call_edge: Color::Rgb(14, 116, 144),
                    ownership_edge: Color::Rgb(21, 128, 61),
                    attack_edge: Color::Rgb(185, 28, 28),
                    cluster: Color::Rgb(213, 219, 229),
                },
            },
        }
    }
}

impl std::fmt::Display for ThemeName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

impl std::str::FromStr for ThemeName {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let normalized: String = s
            .to_lowercase()
            .chars()
            .filter(|character| character.is_ascii_alphanumeric())
            .collect();

        match normalized.as_str() {
            "peregrinenight" | "peregrine" | "night" => Ok(Self::PeregrineNight),
            "falconwatch" | "falcon" | "watch" => Ok(Self::FalconWatch),
            "auditdark" | "audit" => Ok(Self::AuditDark),
            "threatmatrix" | "threat" | "matrix" => Ok(Self::ThreatMatrix),
            "bytecodeember" | "bytecode" | "ember" => Ok(Self::BytecodeEmber),
            "sandboxgraphite" | "sandbox" | "graphite" => Ok(Self::SandboxGraphite),
            "invariantpine" | "invariant" | "pine" | "spec" => Ok(Self::InvariantPine),
            "zeroday" | "zero" | "neon" => Ok(Self::ZeroDay),
            "evidencelight" | "evidence" | "report" => Ok(Self::EvidenceLight),
            "whitebox" | "white" | "light" => Ok(Self::Whitebox),
            _ => Err(format!("Unknown theme: {s}")),
        }
    }
}

/// A theme configuration wrapper providing convenient access to theme colors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Theme {
    /// The selected theme name.
    #[serde(default)]
    pub name: ThemeName,
}

impl Theme {
    /// Create a new theme with the given name.
    #[must_use]
    pub const fn new(name: ThemeName) -> Self {
        Self { name }
    }

    /// Returns the color palette for the current theme.
    #[must_use]
    pub const fn palette(&self) -> ThemePalette {
        self.name.palette()
    }

    /// Returns the syntax palette for the current theme.
    #[must_use]
    pub const fn syntax(&self) -> SyntaxPalette {
        self.name.palette().syntax
    }

    /// Returns the graph palette for the current theme.
    #[must_use]
    pub const fn graph(&self) -> GraphPalette {
        self.name.palette().graph
    }

    /// Check if this is a light theme.
    #[must_use]
    pub fn is_light(&self) -> bool {
        self.palette().is_light()
    }

    /// Check if this is a dark theme.
    #[must_use]
    pub fn is_dark(&self) -> bool {
        self.palette().is_dark()
    }

    /// Cycle to the next theme in the list.
    pub fn next(&mut self) {
        self.name = self.name.next();
    }

    /// Cycle to the previous theme in the list.
    pub fn prev(&mut self) {
        self.name = self.name.prev();
    }
}

impl From<ThemeName> for Theme {
    fn from(name: ThemeName) -> Self {
        Self::new(name)
    }
}

impl std::fmt::Display for Theme {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_themes_have_palettes() {
        for theme in ThemeName::all() {
            let palette = theme.palette();
            assert_ne!(palette.fg, palette.bg);
            assert_ne!(palette.syntax.text, palette.syntax.active_line);
            assert_ne!(palette.graph.node, palette.graph.edge);
        }
    }

    #[test]
    fn test_theme_cycling() {
        let mut theme = ThemeName::PeregrineNight;
        let original = theme;

        for _ in 0..ThemeName::all().len() {
            theme = theme.next();
        }

        assert_eq!(theme, original);
    }

    #[test]
    fn test_theme_cycling_backward() {
        let mut theme = ThemeName::PeregrineNight;
        let original = theme;

        for _ in 0..ThemeName::all().len() {
            theme = theme.prev();
        }

        assert_eq!(theme, original);
    }

    #[test]
    fn test_light_dark_detection() {
        assert!(ThemeName::EvidenceLight.palette().is_light());
        assert!(ThemeName::Whitebox.palette().is_light());

        assert!(ThemeName::PeregrineNight.palette().is_dark());
        assert!(ThemeName::FalconWatch.palette().is_dark());
        assert!(ThemeName::AuditDark.palette().is_dark());
        assert!(ThemeName::ThreatMatrix.palette().is_dark());
        assert!(ThemeName::BytecodeEmber.palette().is_dark());
        assert!(ThemeName::SandboxGraphite.palette().is_dark());
        assert!(ThemeName::InvariantPine.palette().is_dark());
        assert!(ThemeName::ZeroDay.palette().is_dark());
    }

    #[test]
    fn test_display_name() {
        assert_eq!(ThemeName::PeregrineNight.display_name(), "Peregrine Night");
        assert_eq!(ThemeName::ThreatMatrix.display_name(), "Threat Matrix");
        assert_eq!(ThemeName::BytecodeEmber.display_name(), "Bytecode Ember");
    }

    #[test]
    fn test_theme_display_trait() {
        assert_eq!(format!("{}", ThemeName::PeregrineNight), "Peregrine Night");
        assert_eq!(format!("{}", ThemeName::Whitebox), "Whitebox");
    }

    #[test]
    fn test_theme_wrapper() {
        let mut theme = Theme::new(ThemeName::PeregrineNight);
        assert_eq!(theme.name, ThemeName::PeregrineNight);
        assert!(theme.is_dark());

        theme.next();
        assert_eq!(theme.name, ThemeName::FalconWatch);

        theme.prev();
        assert_eq!(theme.name, ThemeName::PeregrineNight);
    }

    #[test]
    fn test_theme_from_name() {
        let theme: Theme = ThemeName::ThreatMatrix.into();
        assert_eq!(theme.name, ThemeName::ThreatMatrix);
    }

    #[test]
    fn test_default_theme() {
        assert_eq!(ThemeName::default(), ThemeName::PeregrineNight);
        assert_eq!(Theme::default().name, ThemeName::PeregrineNight);
    }

    #[test]
    fn test_theme_count() {
        assert_eq!(ThemeName::all().len(), 10);
    }

    #[test]
    fn test_parse_theme_names() {
        assert_eq!(
            "peregrine-night".parse::<ThemeName>().unwrap(),
            ThemeName::PeregrineNight
        );
        assert_eq!(
            "Threat Matrix".parse::<ThemeName>().unwrap(),
            ThemeName::ThreatMatrix
        );
        assert_eq!(
            "bytecode".parse::<ThemeName>().unwrap(),
            ThemeName::BytecodeEmber
        );
        assert_eq!(
            "report".parse::<ThemeName>().unwrap(),
            ThemeName::EvidenceLight
        );
    }
}
