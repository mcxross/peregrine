//! Color palette definitions for Peregrine themes.
//!
//! The palettes are split into core UI colors, syntax highlighting colors,
//! and graph visualization colors so terminal screens remain consistent across
//! audit dashboards, code editors, CFG views, and object lifecycle graphs.

use ratatui::style::Color;

use crate::theme::ThemeName;

/// A semantic color palette for a theme.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThemePalette {
    /// Primary accent color for active elements, links, and focus rings.
    pub accent: Color,
    /// Secondary accent color for less prominent highlights.
    pub secondary: Color,
    /// Main application background color.
    pub bg: Color,
    /// Primary foreground/text color.
    pub fg: Color,
    /// Muted text color for comments, placeholders, timestamps, and metadata.
    pub muted: Color,
    /// Selection or highlighted row background color.
    pub selection: Color,
    /// Error color for critical alerts, failed checks, and dangerous findings.
    pub error: Color,
    /// Warning color for cautions, weak assumptions, and pending approvals.
    pub warning: Color,
    /// Success color for verified, passed, resolved, or safe states.
    pub success: Color,
    /// Informational color for traces, links, and neutral highlights.
    pub info: Color,
    /// Code editor and syntax highlighting colors.
    pub syntax: SyntaxPalette,
    /// Graph visualization colors for call graphs, CFGs, and object lifecycle views.
    pub graph: GraphPalette,
}

impl ThemePalette {
    /// Check if this is a light theme based on background brightness.
    #[must_use]
    pub fn is_light(&self) -> bool {
        if let Color::Rgb(r, g, b) = self.bg {
            let brightness = (u32::from(r) * 299 + u32::from(g) * 587 + u32::from(b) * 114) / 1000;
            brightness > 127
        } else {
            false
        }
    }

    /// Check if this is a dark theme.
    #[must_use]
    pub fn is_dark(&self) -> bool {
        !self.is_light()
    }
}

impl Default for ThemePalette {
    fn default() -> Self {
        ThemeName::default().palette()
    }
}

/// Syntax highlighting colors for terminal code editors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyntaxPalette {
    /// Normal editor text.
    pub text: Color,
    /// Line numbers and gutter labels.
    pub gutter: Color,
    /// Active line background.
    pub active_line: Color,
    /// Comments and documentation comments.
    pub comment: Color,
    /// Language keywords.
    pub keyword: Color,
    /// Types, structs, enums, traits, and modules.
    pub type_name: Color,
    /// Functions, methods, and callable symbols.
    pub function: Color,
    /// Variables, fields, and bindings.
    pub variable: Color,
    /// String literals.
    pub string: Color,
    /// Number and boolean literals.
    pub number: Color,
    /// Operators and punctuation.
    pub operator: Color,
    /// Macros, attributes, annotations, and decorators.
    pub macro_name: Color,
    /// Inserted lines in diffs.
    pub diff_add: Color,
    /// Removed lines in diffs.
    pub diff_delete: Color,
    /// Modified lines in diffs.
    pub diff_change: Color,
    /// Security-sensitive code regions, capabilities, signer paths, or privileged calls.
    pub security_sensitive: Color,
}

/// Graph visualization colors for security analysis views.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GraphPalette {
    /// Default graph node color.
    pub node: Color,
    /// Secondary or less important node color.
    pub node_secondary: Color,
    /// Selected or focused node color.
    pub selected_node: Color,
    /// Entry point node color.
    pub entry_node: Color,
    /// Exit or terminal node color.
    pub exit_node: Color,
    /// Critical finding or high-risk node color.
    pub critical_node: Color,
    /// Warning or medium-risk node color.
    pub warning_node: Color,
    /// Safe, verified, or invariant-holding node color.
    pub safe_node: Color,
    /// Default graph edge color.
    pub edge: Color,
    /// Data-flow edge color.
    pub data_flow: Color,
    /// Control-flow edge color.
    pub control_flow: Color,
    /// Call edge color.
    pub call_edge: Color,
    /// Ownership, object lifecycle, or resource movement edge color.
    pub ownership_edge: Color,
    /// Attack path, exploit route, or suspicious relation color.
    pub attack_edge: Color,
    /// Graph cluster, group, or module boundary color.
    pub cluster: Color,
}
