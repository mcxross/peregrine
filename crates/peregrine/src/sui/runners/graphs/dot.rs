pub fn dot_id(id: &str) -> String {
    format!("\"{}\"", escape_dot(id))
}

pub fn dot_label(label: &str) -> String {
    format!("\"{}\"", escape_dot(label))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DotEdgeStyle {
    pub color: &'static str,
    pub fontcolor: &'static str,
    pub style: &'static str,
    pub penwidth: &'static str,
}

impl DotEdgeStyle {
    pub const fn new(
        color: &'static str,
        fontcolor: &'static str,
        style: &'static str,
        penwidth: &'static str,
    ) -> Self {
        Self {
            color,
            fontcolor,
            style,
            penwidth,
        }
    }
}

pub fn dot_edge_attrs(label: &str, style: DotEdgeStyle) -> String {
    format!(
        "label={}, color=\"{}\", fontcolor=\"{}\", style=\"{}\", penwidth=\"{}\"",
        dot_label(label),
        style.color,
        style.fontcolor,
        style.style,
        style.penwidth
    )
}

pub fn escape_dot(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dot_label_escapes_quotes_and_newlines() {
        assert_eq!(dot_label("a\"b\nc"), "\"a\\\"b\\nc\"");
    }

    #[test]
    fn dot_edge_attrs_includes_semantic_style() {
        let style = DotEdgeStyle::new("#22c55e", "#bbf7d0", "bold", "2.2");

        assert_eq!(
            dot_edge_attrs("direct", style),
            "label=\"direct\", color=\"#22c55e\", fontcolor=\"#bbf7d0\", style=\"bold\", penwidth=\"2.2\""
        );
    }
}
