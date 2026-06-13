use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub enum SourcePrecision {
    ExactExpression,
    Statement,
    Function,
    Module,
    File,
    SummaryArtifact,
    #[default]
    Unknown,
}
