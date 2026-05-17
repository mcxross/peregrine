use std::{fs, path::Path};

use peregrine_indexer::{
    core::{ContextBudget, ContextLevel},
    storage::sqlite::SqliteIndexReader,
    IndexerConfig, SuiMoveIndexer,
};
use rusqlite::Connection;
use tempfile::tempdir;

#[test]
fn package_summaries_pointer_basic_indexes_without_raw_summary_duplication() {
    let root = copy_fixture("package_summaries_pointer_basic");
    let indexer = SuiMoveIndexer::new(IndexerConfig::default());
    let report = indexer
        .index_package(root.path(), "test-run".to_string())
        .expect("index package");

    assert_eq!(report.summary_artifact_count, 8);
    assert_eq!(report.module_count, 4);
    assert!(report.function_count >= 5);
    assert!(report.type_count >= 2);
    assert_eq!(report.operation_count, 0);

    let connection = Connection::open(root.path().join(".peregrine/index.sqlite")).expect("db");
    let root_metadata_files: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM files WHERE kind = 'root_package_metadata'",
            [],
            |row| row.get(0),
        )
        .expect("root metadata file count");
    assert_eq!(root_metadata_files, 1);

    let raw_summary_rows: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM summary_artifacts WHERE raw_summary_json IS NOT NULL",
            [],
            |row| row.get(0),
        )
        .expect("raw summary count");
    assert_eq!(raw_summary_rows, 0);

    let direct_dependency_cards: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM summary_artifacts WHERE materialized_status = 'DirectDependencyCard'",
            [],
            |row| row.get(0),
        )
        .expect("direct dependency card count");
    assert_eq!(direct_dependency_cards, 4);

    let expanded: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM summary_artifacts WHERE materialized_status IN ('ExpandedModule', 'ExpandedSymbol')",
            [],
            |row| row.get(0),
        )
        .expect("expanded count");
    assert_eq!(expanded, 0);
}

#[test]
fn lazy_materialization_expands_only_requested_dependency_module() {
    let root = copy_fixture("package_summaries_pointer_basic");
    let indexer = SuiMoveIndexer::new(IndexerConfig::default());
    indexer
        .index_package(root.path(), "test-run".to_string())
        .expect("index package");
    let db_path = root.path().join(".peregrine/index.sqlite");

    let card = indexer
        .materialize_summary_module(
            &db_path,
            "pyth",
            "price_info",
            ContextBudget {
                max_tokens_estimate: 120,
                level: ContextLevel::Level0,
                ..ContextBudget::default()
            },
        )
        .expect("materialize module");
    assert_eq!(card.package_alias, "pyth");
    assert_eq!(card.module_name, "price_info");
    assert!(matches!(
        card.materialized_status.as_str(),
        "ExpandedModule"
    ));

    let connection = Connection::open(db_path).expect("db");
    let deepbook_status: String = connection
        .query_row(
            "SELECT materialized_status FROM summary_artifacts WHERE package_alias = 'deepbook_margin' AND module_name = 'margin_pool'",
            [],
            |row| row.get(0),
        )
        .expect("deepbook status");
    assert_eq!(deepbook_status, "DirectDependencyCard");
}

#[test]
fn malformed_summary_creates_diagnostic_without_blocking_valid_pointers() {
    let temp = tempdir().expect("temp");
    fs::write(
        temp.path().join("Move.toml"),
        "[package]\nname = \"broken_summary\"\n",
    )
    .expect("manifest");
    fs::create_dir_all(temp.path().join("package_summaries/broken_summary")).expect("summaries");
    fs::write(
        temp.path().join("package_summaries/broken_summary/good.json"),
        r#"{
          "id": { "address": "broken_summary", "name": "good" },
          "immediate_dependencies": [],
          "structs": {},
          "functions": { "ping": { "visibility": "Public", "entry": false, "parameters": [], "return_": [] } }
        }"#,
    )
    .expect("good summary");
    fs::write(
        temp.path()
            .join("package_summaries/broken_summary/bad.json"),
        "{ not valid json",
    )
    .expect("bad summary");

    let report = SuiMoveIndexer::new(IndexerConfig::default())
        .index_package(temp.path(), "test-run".to_string())
        .expect("index package");

    assert_eq!(report.summary_artifact_count, 2);
    assert_eq!(report.module_count, 1);
    assert!(report.diagnostic_count >= 1);
}

#[test]
fn function_context_is_compact_and_neutral() {
    let root = copy_fixture("package_summaries_pointer_basic");
    let indexer = SuiMoveIndexer::new(IndexerConfig::default());
    let report = indexer
        .index_package(root.path(), "test-run".to_string())
        .expect("index package");
    let db_path = root.path().join(".peregrine/index.sqlite");
    let reader = SqliteIndexReader::open(&db_path).expect("reader");
    let symbol = reader
        .search_symbols(&report.package_id, "deposit", &ContextBudget::default())
        .expect("search")
        .into_iter()
        .find(|symbol| symbol.kind == "function")
        .expect("deposit function");
    let context = indexer
        .get_function_context(
            &db_path,
            &symbol.id,
            &ContextBudget {
                max_tokens_estimate: 1000,
                level: ContextLevel::Level1,
                ..ContextBudget::default()
            },
        )
        .expect("function context");

    assert!(context.estimated_tokens <= context.budget_tokens);
    assert!(context
        .card
        .top_tags
        .contains(&"public_entry_detected".to_string()));
    let serialized = serde_json::to_string(&context).expect("json");
    assert!(!serialized.contains("vulnerable"));
    assert!(!serialized.contains("missing_authorization"));
    assert!(!serialized.contains("unguarded_transfer"));
}

#[test]
fn source_spans_and_required_query_paths_are_available() {
    let root = copy_fixture("package_summaries_pointer_basic");
    let indexer = SuiMoveIndexer::new(IndexerConfig::default());
    let report = indexer
        .index_package(root.path(), "test-run".to_string())
        .expect("index package");
    let db_path = root.path().join(".peregrine/index.sqlite");
    let reader = SqliteIndexReader::open(&db_path).expect("reader");
    let deposit = reader
        .search_symbols(&report.package_id, "deposit", &ContextBudget::default())
        .expect("search")
        .into_iter()
        .find(|symbol| symbol.kind == "function")
        .expect("deposit function");

    let budget = ContextBudget {
        level: ContextLevel::Level2,
        include_source: true,
        max_tokens_estimate: 1_500,
        ..ContextBudget::default()
    };
    let context = indexer
        .get_function_body(&db_path, &deposit.id, &budget)
        .expect("function body");
    assert!(context.card.source_span.file_id.is_some());
    assert_eq!(context.card.source_span.start_line, Some(3));
    assert!(!context.source_excerpts.is_empty());
    assert!(context.estimated_tokens <= context.budget_tokens);

    assert!(indexer
        .get_function_operations(&db_path, &deposit.id, &budget)
        .expect("operations")
        .is_empty());
    assert!(indexer
        .get_function_field_reads(&db_path, &deposit.id)
        .expect("field reads")
        .is_empty());
    assert!(indexer
        .get_reachable_callees(&db_path, &deposit.id, 2, &budget)
        .expect("reachable callees")
        .is_empty());
    assert!(!indexer
        .get_public_entry_functions(&db_path, &report.package_id)
        .expect("entry functions")
        .is_empty());
}

fn copy_fixture(name: &str) -> tempfile::TempDir {
    let temp = tempdir().expect("tempdir");
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sui")
        .join(name);
    copy_dir(&source, temp.path());
    temp
}

fn copy_dir(source: &Path, target: &Path) {
    for entry in fs::read_dir(source).expect("read fixture") {
        let entry = entry.expect("fixture entry");
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());
        if source_path.is_dir() {
            fs::create_dir_all(&target_path).expect("create dir");
            copy_dir(&source_path, &target_path);
        } else {
            fs::copy(&source_path, &target_path).expect("copy file");
        }
    }
}
