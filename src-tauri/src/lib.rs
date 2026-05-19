mod commands;
mod file_preview;
pub mod helper_args;
mod menu;
mod state;

use state::IndexerCommandState;

pub(crate) fn validated_move_project_name(project_name: &str) -> Result<String, String> {
    let project_name = project_name.trim();

    if project_name.is_empty() {
        return Err("Project name cannot be empty.".to_string());
    }

    if project_name.len() > 128 {
        return Err("Project name is too long.".to_string());
    }

    let mut characters = project_name.chars();
    let Some(first) = characters.next() else {
        return Err("Project name cannot be empty.".to_string());
    };

    if !(first == '_' || first.is_ascii_alphabetic()) {
        return Err("Project name must start with a letter or underscore.".to_string());
    }

    if !characters.all(|character| character == '_' || character.is_ascii_alphanumeric()) {
        return Err("Project name can only contain letters, numbers, and underscores.".to_string());
    }

    Ok(project_name.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .menu(menu::app_menu)
        .on_menu_event(menu::handle_menu_event)
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(IndexerCommandState::default())
        .invoke_handler(tauri::generate_handler![
            commands::indexer::greet,
            commands::indexer::index_package,
            commands::indexer::reindex_package,
            commands::indexer::cancel_index,
            commands::indexer::get_package_overview,
            commands::indexer::get_module_context,
            commands::indexer::get_type_context,
            commands::indexer::get_function_context,
            commands::indexer::get_function_body,
            commands::indexer::get_function_operations,
            commands::indexer::get_function_callers,
            commands::indexer::get_function_callees,
            commands::indexer::get_reachable_callees,
            commands::indexer::get_function_field_reads,
            commands::indexer::get_function_field_writes,
            commands::indexer::get_call_graph,
            commands::indexer::search_symbols,
            commands::indexer::get_operations_by_tag,
            commands::indexer::get_functions_by_tag,
            commands::indexer::get_public_entry_functions,
            commands::indexer::get_diagnostics,
            commands::indexer::get_context_pack,
            commands::indexer::materialize_summary_module,
            commands::indexer::materialize_summary_symbol,
            commands::files::load_package_tree,
            commands::files::load_package_tree_details,
            commands::files::load_move_graphs,
            commands::files::load_move_state_access_graph,
            commands::sui::create_move_project,
            commands::sui::import_move_package_by_id,
            commands::files::move_project_path_exists,
            commands::files::load_file_preview,
            commands::files::save_text_file,
            commands::files::save_graph_png,
            commands::sui::build_move_package,
            commands::sui::run_security_command,
            commands::sui::run_movy_fuzz,
            commands::sui::run_formal_verification,
            commands::sui::run_security_script,
            commands::files::analyze_move_package,
            commands::files::load_move_bytecode_view,
            commands::sui::check_sui_adapter,
            commands::sui::get_sui_adapter_settings,
            commands::sui::save_sui_adapter_settings,
            commands::metadata::load_project_metadata,
            commands::metadata::save_project_metadata,
            commands::ollama::list_ollama_models,
            commands::ollama::chat_with_ollama,
            commands::ollama::preload_ollama_model,
            commands::ollama::stream_chat_with_ollama
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::validated_move_project_name;

    #[test]
    fn move_project_name_accepts_move_identifiers() {
        assert_eq!(
            validated_move_project_name(" savings_vault_2 ").expect("valid project name"),
            "savings_vault_2"
        );
    }

    #[test]
    fn move_project_name_rejects_paths() {
        assert_eq!(
            validated_move_project_name("../savings").expect_err("path-like name should fail"),
            "Project name must start with a letter or underscore."
        );
        assert_eq!(
            validated_move_project_name("savings-vault").expect_err("hyphenated name should fail"),
            "Project name can only contain letters, numbers, and underscores."
        );
    }
}
