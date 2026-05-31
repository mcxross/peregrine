use crate::state::IndexerCommandState;
use peregrine_indexer::{
    IndexerConfig, SuiMoveIndexer,
    core::{ContextBudget, Diagnostic as IndexDiagnostic, Operation as IndexOperation},
    sui::model::{
        ContextPack, FunctionContext, GraphView, IndexReport, ModuleContext, ModuleSummaryCard,
        PackageOverview, SymbolResult, TypeContext,
    },
    tauri::events as index_events,
};
use serde::Serialize;
use std::path::PathBuf;
use tauri::Emitter;

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct IndexEventPayload {
    run_id: String,
    message: String,
    package_id: Option<String>,
}

#[tauri::command]
pub(crate) fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Peregrine!", name)
}

#[tauri::command]
pub(crate) async fn index_package(
    app: tauri::AppHandle,
    state: tauri::State<'_, IndexerCommandState>,
    root_path: String,
    run_id: Option<String>,
) -> Result<IndexReport, String> {
    let run_id = run_id.unwrap_or_else(new_index_run_id);
    emit_index_event(
        &app,
        index_events::INDEX_STARTED,
        &run_id,
        "Index started.",
        None,
    );
    emit_index_event(
        &app,
        index_events::INDEX_PROGRESS,
        &run_id,
        "Preparing Sui Move package index.",
        None,
    );
    emit_index_event(
        &app,
        index_events::INDEX_DISCOVERING_SUMMARIES,
        &run_id,
        "Discovering package summaries.",
        None,
    );
    emit_index_event(
        &app,
        index_events::INDEX_EXTRACTING_SUMMARY_POINTERS,
        &run_id,
        "Extracting summary artifact pointers.",
        None,
    );
    emit_index_event(
        &app,
        index_events::INDEX_MATERIALIZING_ROOT_SUMMARIES,
        &run_id,
        "Materializing root summary cards.",
        None,
    );
    emit_index_event(
        &app,
        index_events::INDEX_COMPILING,
        &run_id,
        "Reading compiler build artifacts.",
        None,
    );
    emit_index_event(
        &app,
        index_events::INDEX_ENRICHING_FULL,
        &run_id,
        "Checking for Full mode build artifacts.",
        None,
    );
    emit_index_event(
        &app,
        index_events::INDEX_PERSISTING,
        &run_id,
        "Persisting normalized index.",
        None,
    );

    let run_id_for_task = run_id.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        SuiMoveIndexer::new(IndexerConfig::default()).index_package(root_path, run_id_for_task)
    })
    .await
    .map_err(|error| format!("Could not join index task: {error}"))?
    .map_err(|error| error.to_string());

    match result {
        Ok(report) => {
            if !state
                .canceled_runs
                .lock()
                .map_err(|_| "Could not lock index cancellation state.".to_string())?
                .contains(&run_id)
            {
                *state
                    .active_db_path
                    .lock()
                    .map_err(|_| "Could not lock active index state.".to_string())? =
                    Some(PathBuf::from(&report.db_path));
            }
            emit_index_event(
                &app,
                index_events::INDEX_COMPLETED,
                &run_id,
                "Index completed.",
                Some(report.package_id.clone()),
            );
            Ok(report)
        }
        Err(error) => {
            emit_index_event(&app, index_events::INDEX_FAILED, &run_id, &error, None);
            Err(error)
        }
    }
}

#[tauri::command]
pub(crate) async fn reindex_package(
    app: tauri::AppHandle,
    state: tauri::State<'_, IndexerCommandState>,
    package_id: String,
) -> Result<IndexReport, String> {
    let db_path = active_index_db_path(&state)?;
    let run_id = new_index_run_id();
    emit_index_event(
        &app,
        index_events::INDEX_STARTED,
        &run_id,
        "Reindex started.",
        Some(package_id.clone()),
    );
    emit_index_event(
        &app,
        index_events::INDEX_PROGRESS,
        &run_id,
        "Preparing Sui Move package reindex.",
        Some(package_id.clone()),
    );
    emit_index_event(
        &app,
        index_events::INDEX_DISCOVERING_SUMMARIES,
        &run_id,
        "Discovering package summaries.",
        Some(package_id.clone()),
    );
    emit_index_event(
        &app,
        index_events::INDEX_COMPILING,
        &run_id,
        "Reading compiler build artifacts.",
        Some(package_id.clone()),
    );
    let run_id_for_task = run_id.clone();
    let package_id_for_task = package_id.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        SuiMoveIndexer::new(IndexerConfig::default()).reindex_package(
            db_path,
            &package_id_for_task,
            run_id_for_task,
        )
    })
    .await
    .map_err(|error| format!("Could not join reindex task: {error}"))?
    .map_err(|error| error.to_string())?;
    *state
        .active_db_path
        .lock()
        .map_err(|_| "Could not lock active index state.".to_string())? =
        Some(PathBuf::from(&result.db_path));
    emit_index_event(
        &app,
        index_events::INDEX_COMPLETED,
        &run_id,
        "Reindex completed.",
        Some(result.package_id.clone()),
    );
    Ok(result)
}

#[tauri::command]
pub(crate) fn cancel_index(
    state: tauri::State<'_, IndexerCommandState>,
    run_id: String,
) -> Result<bool, String> {
    state
        .canceled_runs
        .lock()
        .map_err(|_| "Could not lock index cancellation state.".to_string())?
        .insert(run_id);
    Ok(true)
}

#[tauri::command]
pub(crate) fn get_package_overview(
    state: tauri::State<'_, IndexerCommandState>,
    package_id: String,
) -> Result<PackageOverview, String> {
    let db_path = active_index_db_path(&state)?;
    SuiMoveIndexer::new(IndexerConfig::default())
        .get_package_overview(db_path, &package_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn get_module_context(
    state: tauri::State<'_, IndexerCommandState>,
    module_id: String,
) -> Result<ModuleContext, String> {
    let db_path = active_index_db_path(&state)?;
    SuiMoveIndexer::new(IndexerConfig::default())
        .get_module_context(db_path, &module_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn get_type_context(
    state: tauri::State<'_, IndexerCommandState>,
    type_id: String,
) -> Result<TypeContext, String> {
    let db_path = active_index_db_path(&state)?;
    SuiMoveIndexer::new(IndexerConfig::default())
        .get_type_context(db_path, &type_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn get_function_context(
    state: tauri::State<'_, IndexerCommandState>,
    function_id: String,
    budget: ContextBudget,
) -> Result<FunctionContext, String> {
    let db_path = active_index_db_path(&state)?;
    SuiMoveIndexer::new(IndexerConfig::default())
        .get_function_context(db_path, &function_id, &budget)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn get_function_body(
    state: tauri::State<'_, IndexerCommandState>,
    function_id: String,
    budget: ContextBudget,
) -> Result<FunctionContext, String> {
    let db_path = active_index_db_path(&state)?;
    SuiMoveIndexer::new(IndexerConfig::default())
        .get_function_body(db_path, &function_id, &budget)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn get_function_operations(
    state: tauri::State<'_, IndexerCommandState>,
    function_id: String,
    budget: ContextBudget,
) -> Result<Vec<IndexOperation>, String> {
    let db_path = active_index_db_path(&state)?;
    SuiMoveIndexer::new(IndexerConfig::default())
        .get_function_operations(db_path, &function_id, &budget)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn get_function_callers(
    state: tauri::State<'_, IndexerCommandState>,
    function_id: String,
    budget: ContextBudget,
) -> Result<Vec<String>, String> {
    let db_path = active_index_db_path(&state)?;
    SuiMoveIndexer::new(IndexerConfig::default())
        .get_function_callers(db_path, &function_id, &budget)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn get_function_callees(
    state: tauri::State<'_, IndexerCommandState>,
    function_id: String,
    budget: ContextBudget,
) -> Result<Vec<String>, String> {
    let db_path = active_index_db_path(&state)?;
    SuiMoveIndexer::new(IndexerConfig::default())
        .get_function_callees(db_path, &function_id, &budget)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn get_reachable_callees(
    state: tauri::State<'_, IndexerCommandState>,
    function_id: String,
    depth: usize,
    budget: ContextBudget,
) -> Result<Vec<String>, String> {
    let db_path = active_index_db_path(&state)?;
    SuiMoveIndexer::new(IndexerConfig::default())
        .get_reachable_callees(db_path, &function_id, depth, &budget)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn get_function_field_reads(
    state: tauri::State<'_, IndexerCommandState>,
    function_id: String,
) -> Result<Vec<String>, String> {
    let db_path = active_index_db_path(&state)?;
    SuiMoveIndexer::new(IndexerConfig::default())
        .get_function_field_reads(db_path, &function_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn get_function_field_writes(
    state: tauri::State<'_, IndexerCommandState>,
    function_id: String,
) -> Result<Vec<String>, String> {
    let db_path = active_index_db_path(&state)?;
    SuiMoveIndexer::new(IndexerConfig::default())
        .get_function_field_writes(db_path, &function_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn get_call_graph(
    state: tauri::State<'_, IndexerCommandState>,
    function_id: String,
    depth: usize,
    budget: ContextBudget,
) -> Result<GraphView, String> {
    let db_path = active_index_db_path(&state)?;
    SuiMoveIndexer::new(IndexerConfig::default())
        .get_call_graph(db_path, &function_id, depth, &budget)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn search_symbols(
    state: tauri::State<'_, IndexerCommandState>,
    package_id: String,
    query: String,
    budget: ContextBudget,
) -> Result<Vec<SymbolResult>, String> {
    let db_path = active_index_db_path(&state)?;
    SuiMoveIndexer::new(IndexerConfig::default())
        .search_symbols(db_path, &package_id, &query, &budget)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn get_operations_by_tag(
    state: tauri::State<'_, IndexerCommandState>,
    package_id: String,
    tag: String,
    budget: ContextBudget,
) -> Result<Vec<IndexOperation>, String> {
    let db_path = active_index_db_path(&state)?;
    SuiMoveIndexer::new(IndexerConfig::default())
        .get_operations_by_tag(db_path, &package_id, &tag, &budget)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn get_functions_by_tag(
    state: tauri::State<'_, IndexerCommandState>,
    package_id: String,
    tag: String,
    budget: ContextBudget,
) -> Result<Vec<SymbolResult>, String> {
    let db_path = active_index_db_path(&state)?;
    SuiMoveIndexer::new(IndexerConfig::default())
        .get_functions_by_tag(db_path, &package_id, &tag, &budget)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn get_public_entry_functions(
    state: tauri::State<'_, IndexerCommandState>,
    package_id: String,
) -> Result<Vec<SymbolResult>, String> {
    let db_path = active_index_db_path(&state)?;
    SuiMoveIndexer::new(IndexerConfig::default())
        .get_public_entry_functions(db_path, &package_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn get_diagnostics(
    state: tauri::State<'_, IndexerCommandState>,
    package_id: String,
) -> Result<Vec<IndexDiagnostic>, String> {
    let db_path = active_index_db_path(&state)?;
    SuiMoveIndexer::new(IndexerConfig::default())
        .get_diagnostics(db_path, &package_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn get_context_pack(
    state: tauri::State<'_, IndexerCommandState>,
    target_id: String,
    budget: ContextBudget,
) -> Result<ContextPack, String> {
    let db_path = active_index_db_path(&state)?;
    SuiMoveIndexer::new(IndexerConfig::default())
        .get_context_pack(db_path, &target_id, &budget)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn materialize_summary_module(
    state: tauri::State<'_, IndexerCommandState>,
    package_alias: String,
    module_name: String,
    budget: ContextBudget,
) -> Result<ModuleSummaryCard, String> {
    let db_path = active_index_db_path(&state)?;
    SuiMoveIndexer::new(IndexerConfig::default())
        .materialize_summary_module(db_path, &package_alias, &module_name, budget)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn materialize_summary_symbol(
    state: tauri::State<'_, IndexerCommandState>,
    package_alias: String,
    module_name: String,
    symbol_name: String,
    budget: ContextBudget,
) -> Result<ModuleSummaryCard, String> {
    let db_path = active_index_db_path(&state)?;
    SuiMoveIndexer::new(IndexerConfig::default())
        .materialize_summary_symbol(db_path, &package_alias, &module_name, &symbol_name, budget)
        .map_err(|error| error.to_string())
}

fn active_index_db_path(state: &tauri::State<'_, IndexerCommandState>) -> Result<PathBuf, String> {
    state
        .active_db_path
        .lock()
        .map_err(|_| "Could not lock active index state.".to_string())?
        .clone()
        .ok_or_else(|| "No active Peregrine index. Run index_package first.".to_string())
}

fn new_index_run_id() -> String {
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();
    format!("index-{millis}")
}

fn emit_index_event(
    app: &tauri::AppHandle,
    event: &str,
    run_id: &str,
    message: &str,
    package_id: Option<String>,
) {
    let payload = IndexEventPayload {
        run_id: run_id.to_string(),
        message: message.to_string(),
        package_id,
    };
    let _ = app.emit(event, payload.clone());
    let _ = app.emit(index_events::INDEX_PROGRESS, payload);
}
