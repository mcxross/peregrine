use peregrine_analysis::{AnalysisReport, GraphKind};
use peregrine_sui_mcp_protocol::{MoveProjectGraphs, MoveStateAccessGraph};
use rmcp::ErrorData;
use serde::de::DeserializeOwned;

pub(crate) fn legacy_project_graphs(
    report: &AnalysisReport,
) -> Result<MoveProjectGraphs, ErrorData> {
    Ok(MoveProjectGraphs {
        call_graph: legacy_graph(report, GraphKind::CALL)?,
        type_graph: legacy_graph(report, GraphKind::TYPE)?,
        state_access_graph: legacy_graph(report, GraphKind::STATE_ACCESS)?,
    })
}

pub(crate) fn legacy_state_graph(
    report: &AnalysisReport,
) -> Result<MoveStateAccessGraph, ErrorData> {
    legacy_graph(report, GraphKind::STATE_ACCESS)
}

fn legacy_graph<T>(report: &AnalysisReport, kind: &str) -> Result<T, ErrorData>
where
    T: DeserializeOwned,
{
    report
        .graphs
        .iter()
        .find(|graph| graph.kind.0 == kind)
        .and_then(|graph| graph.metadata.get("legacyGraph"))
        .cloned()
        .ok_or_else(|| {
            ErrorData::invalid_params(
                format!("analysis engine did not produce `{kind}` graph"),
                None,
            )
        })
        .and_then(|value| {
            serde_json::from_value(value)
                .map_err(|error| ErrorData::internal_error(error.to_string(), None))
        })
}
