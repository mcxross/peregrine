use std::{
    fs,
    path::{Path, PathBuf},
};

use peregrine_indexer::{
    core::{ContextBudget, ContextLevel, OperationKind},
    storage::sqlite::SqliteIndexReader,
    sui::model::IndexReport,
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
    let root = copy_fixture("package_summaries_lazy_materialization");
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
    let root = copy_fixture("package_summaries_malformed");

    let report = SuiMoveIndexer::new(IndexerConfig::default())
        .index_package(root.path(), "test-run".to_string())
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

#[test]
fn bytecode_full_mode_extracts_operations_edges_and_source_maps() {
    let root = copy_fixture("bytecode_full_mode");
    let indexer = SuiMoveIndexer::new(IndexerConfig::default());
    let report = indexer
        .index_package(root.path(), "bytecode-run".to_string())
        .expect("index package");
    let db_path = root.path().join(".peregrine/index.sqlite");
    assert!(report.operation_count > 0);

    let deposit = find_function(&db_path, &report.package_id, "deposit");
    let budget = ContextBudget {
        max_tokens_estimate: 3_000,
        level: ContextLevel::Level2,
        include_source: true,
        max_operations: 256,
        ..ContextBudget::default()
    };
    let context = indexer
        .get_function_context(&db_path, &deposit.id, &budget)
        .expect("function context");
    let operations = indexer
        .get_function_operations(&db_path, &deposit.id, &budget)
        .expect("operations");

    assert!(operations.iter().any(|op| op.kind == OperationKind::Call));
    assert!(operations.iter().any(|op| op.kind == OperationKind::Assert));
    assert!(operations
        .iter()
        .any(|op| op.kind == OperationKind::ReadField));
    assert!(operations
        .iter()
        .any(|op| op.kind == OperationKind::WriteField));
    assert!(operations.iter().any(|op| op.kind == OperationKind::Add));
    assert!(context
        .card
        .top_tags
        .contains(&"public_function_detected".to_string()));
    assert!(context
        .card
        .top_tags
        .contains(&"tx_context_parameter_detected".to_string()));
    assert!(context
        .card
        .top_tags
        .contains(&"mutable_reference_parameter_detected".to_string()));
    assert!(!context.field_reads.is_empty());
    assert!(!context.field_writes.is_empty());
    assert!(!context.source_excerpts.is_empty());

    let connection = Connection::open(db_path).expect("db");
    let source_mapped_operations: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM operations WHERE source_span_json LIKE '%ExactExpression%'",
            [],
            |row| row.get(0),
        )
        .expect("source mapped operations");
    assert!(source_mapped_operations > 0);
}

#[test]
fn bytecode_call_graph_and_operation_order_are_bytecode_backed() {
    let root = copy_fixture("bytecode_full_mode");
    let indexer = SuiMoveIndexer::new(IndexerConfig::default());
    let report = indexer
        .index_package(root.path(), "bytecode-run".to_string())
        .expect("index package");
    let db_path = root.path().join(".peregrine/index.sqlite");
    let budget = ContextBudget {
        max_tokens_estimate: 4_000,
        level: ContextLevel::Level2,
        max_call_depth: 2,
        max_callees: 16,
        max_operations: 256,
        ..ContextBudget::default()
    };

    let wrapper = find_function(&db_path, &report.package_id, "wrapper");
    let send = find_function(&db_path, &report.package_id, "send");
    let reachable = indexer
        .get_reachable_callees(&db_path, &wrapper.id, 2, &budget)
        .expect("reachable callees");
    assert!(reachable.contains(&send.id));
    assert!(reachable
        .iter()
        .any(|callee| callee.contains("::transfer::public_transfer")));

    let transfer_ops = indexer
        .get_operations_by_tag(
            &db_path,
            &report.package_id,
            "transfer_api_call_detected",
            &budget,
        )
        .expect("transfer operations");
    assert!(transfer_ops.iter().any(|op| op
        .target
        .as_deref()
        .is_some_and(|target| target.contains("::transfer::public_transfer"))));

    let transfer_then_assert = find_function(&db_path, &report.package_id, "transfer_then_assert");
    let operations = indexer
        .get_function_operations(&db_path, &transfer_then_assert.id, &budget)
        .expect("operations");
    let transfer_index = operations
        .iter()
        .position(|op| {
            op.kind == OperationKind::Call
                && op
                    .target
                    .as_deref()
                    .is_some_and(|target| target.contains("::transfer::public_transfer"))
        })
        .expect("transfer call");
    let assert_index = operations
        .iter()
        .position(|op| op.kind == OperationKind::Assert)
        .expect("assert op");
    assert!(transfer_index < assert_index);
}

#[test]
fn bytecode_parity_every_call_operation_has_call_edge() {
    let root = copy_fixture("bytecode_full_mode");
    let report = SuiMoveIndexer::new(IndexerConfig::default())
        .index_package(root.path(), "bytecode-run".to_string())
        .expect("index package");
    let db_path = root.path().join(".peregrine/index.sqlite");
    let connection = Connection::open(db_path).expect("db");

    let call_operations: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM operations WHERE package_id = ?1 AND kind = 'Call'",
            [&report.package_id],
            |row| row.get(0),
        )
        .expect("call operation count");
    let call_edges: i64 = connection
        .query_row(
            "SELECT COUNT(DISTINCT operation_id) FROM edges WHERE package_id = ?1 AND edge_type = 'Calls' AND operation_id IS NOT NULL",
            [&report.package_id],
            |row| row.get(0),
        )
        .expect("call edge count");
    assert_eq!(call_edges, call_operations);

    let dangling_call_edges: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM edges LEFT JOIN operations ON operations.id = edges.operation_id WHERE edges.package_id = ?1 AND edges.edge_type = 'Calls' AND edges.operation_id IS NOT NULL AND operations.id IS NULL",
            [&report.package_id],
            |row| row.get(0),
        )
        .expect("dangling call edges");
    assert_eq!(dangling_call_edges, 0);

    let control_flow_edges: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM edges WHERE package_id = ?1 AND edge_type = 'ControlFlow'",
            [&report.package_id],
            |row| row.get(0),
        )
        .expect("control flow edges");
    assert!(control_flow_edges > 0);
}

#[test]
fn compact_context_trimming_preserves_high_signal_operations() {
    let root = copy_fixture("bytecode_full_mode");
    let indexer = SuiMoveIndexer::new(IndexerConfig::default());
    let report = indexer
        .index_package(root.path(), "bytecode-run".to_string())
        .expect("index package");
    let db_path = root.path().join(".peregrine/index.sqlite");
    let transfer_then_assert = find_function(&db_path, &report.package_id, "transfer_then_assert");

    let context = indexer
        .get_function_context(
            &db_path,
            &transfer_then_assert.id,
            &ContextBudget {
                max_tokens_estimate: 900,
                level: ContextLevel::Level2,
                include_source: true,
                max_operations: 256,
                ..ContextBudget::default()
            },
        )
        .expect("function context");

    assert!(context.trimmed);
    assert!(!context.trim_reasons.is_empty());
    assert!(context.estimated_tokens <= context.budget_tokens);
    assert!(context
        .operations
        .iter()
        .any(|op| op.kind == OperationKind::Assert));
    assert!(context.operations.iter().any(|op| {
        op.kind == OperationKind::Call
            && op
                .target
                .as_deref()
                .is_some_and(|target| target.contains("::transfer::public_transfer"))
    }));
}

#[test]
fn generated_summary_shapes_normalize_types_tags_and_dependency_cards() {
    let root = copy_fixture("bytecode_full_mode");
    let indexer = SuiMoveIndexer::new(IndexerConfig::default());
    let report = indexer
        .index_package(root.path(), "bytecode-run".to_string())
        .expect("index package");
    let db_path = root.path().join(".peregrine/index.sqlite");
    let reader = SqliteIndexReader::open(&db_path).expect("reader");

    let vault = reader
        .search_symbols(&report.package_id, "Vault", &ContextBudget::default())
        .expect("search")
        .into_iter()
        .find(|symbol| symbol.kind == "type")
        .expect("vault type");
    let type_context = indexer
        .get_type_context(&db_path, &vault.id)
        .expect("type context");
    assert_eq!(type_context.type_def.abilities, vec!["key", "store"]);
    assert!(type_context
        .type_def
        .fields
        .iter()
        .any(|field| field.name == "id" && field.type_name == "sui::object::UID"));

    let key_types = indexer
        .get_functions_by_tag(
            &db_path,
            &report.package_id,
            "public_function_detected",
            &ContextBudget::default(),
        )
        .expect("functions by tag");
    assert!(key_types
        .iter()
        .any(|function| function.full_name.ends_with("deposit")));

    let deposit = find_function(&db_path, &report.package_id, "deposit");
    let deposit_context = indexer
        .get_function_context(&db_path, &deposit.id, &ContextBudget::default())
        .expect("deposit context");
    assert!(deposit_context
        .outline
        .params
        .iter()
        .any(|param| param.type_name == "&mut bytecode_fixture::vault::Vault"));
    assert!(deposit_context
        .outline
        .params
        .iter()
        .any(|param| param.type_name == "&mut sui::tx_context::TxContext"));

    let connection = Connection::open(db_path).expect("db");
    let direct_dependency_cards: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM summary_artifacts WHERE materialized_status = 'DirectDependencyCard'",
            [],
            |row| row.get(0),
        )
        .expect("direct dependency cards");
    assert_eq!(direct_dependency_cards, 3);
}

#[test]
fn lazy_symbol_materialization_expands_only_requested_symbol() {
    let root = copy_fixture("bytecode_full_mode");
    let indexer = SuiMoveIndexer::new(IndexerConfig::default());
    indexer
        .index_package(root.path(), "bytecode-run".to_string())
        .expect("index package");
    let db_path = root.path().join(".peregrine/index.sqlite");

    let card = indexer
        .materialize_summary_symbol(
            &db_path,
            "sui",
            "transfer",
            "public_transfer",
            ContextBudget {
                max_tokens_estimate: 180,
                level: ContextLevel::Level0,
                ..ContextBudget::default()
            },
        )
        .expect("materialize symbol");
    assert_eq!(card.materialized_status, "ExpandedSymbol");
    assert!(card.estimated_tokens <= card.budget_tokens);
    let serialized = serde_json::to_string(&card.card).expect("json");
    assert!(serialized.contains("public_transfer"));
    assert!(!serialized.contains("debug_raw_summary_json"));

    let pointer = indexer
        .get_summary_artifact_pointer(&db_path, "sui", "object")
        .expect("pointer")
        .expect("object pointer");
    assert_eq!(pointer.materialized_status, "DirectDependencyCard");
}

#[test]
fn context_levels_source_budget_and_call_depth_limits_are_enforced() {
    let root = copy_fixture("bytecode_full_mode");
    let indexer = SuiMoveIndexer::new(IndexerConfig::default());
    let report = indexer
        .index_package(root.path(), "bytecode-run".to_string())
        .expect("index package");
    let db_path = root.path().join(".peregrine/index.sqlite");

    let wrapper = find_function(&db_path, &report.package_id, "wrapper");
    let send = find_function(&db_path, &report.package_id, "send");
    let level0 = indexer
        .get_function_context(
            &db_path,
            &wrapper.id,
            &ContextBudget {
                level: ContextLevel::Level0,
                max_tokens_estimate: 1_000,
                ..ContextBudget::default()
            },
        )
        .expect("level0");
    assert!(level0.operations.is_empty());

    let level2 = indexer
        .get_function_context(
            &db_path,
            &wrapper.id,
            &ContextBudget {
                level: ContextLevel::Level2,
                max_tokens_estimate: 2_000,
                max_operations: 64,
                ..ContextBudget::default()
            },
        )
        .expect("level2");
    assert!(!level2.operations.is_empty());
    assert!(level2.estimated_tokens >= level0.estimated_tokens);

    let level3 = indexer
        .get_function_context(
            &db_path,
            &wrapper.id,
            &ContextBudget {
                level: ContextLevel::Level3,
                max_tokens_estimate: 3_000,
                max_call_depth: 2,
                max_callees: 16,
                ..ContextBudget::default()
            },
        )
        .expect("level3");
    assert!(level3
        .reachable_callees
        .iter()
        .any(|callee| callee.contains("::transfer::public_transfer")));

    let shallow = indexer
        .get_reachable_callees(
            &db_path,
            &wrapper.id,
            2,
            &ContextBudget {
                max_call_depth: 1,
                max_callees: 16,
                ..ContextBudget::default()
            },
        )
        .expect("shallow callees");
    assert!(shallow.contains(&send.id));
    assert!(!shallow
        .iter()
        .any(|callee| callee.contains("::transfer::public_transfer")));

    let graph = indexer
        .get_call_graph(
            &db_path,
            &wrapper.id,
            2,
            &ContextBudget {
                max_call_depth: 1,
                max_callees: 16,
                ..ContextBudget::default()
            },
        )
        .expect("graph");
    assert!(graph.trimmed);
    assert!(graph
        .trim_reasons
        .contains(&"call_depth_limited".to_string()));

    let deposit = find_function(&db_path, &report.package_id, "deposit");
    let excerpt_context = indexer
        .get_function_context(
            &db_path,
            &deposit.id,
            &ContextBudget {
                level: ContextLevel::Level2,
                include_source: true,
                max_source_excerpt_lines: 2,
                max_tokens_estimate: 2_000,
                ..ContextBudget::default()
            },
        )
        .expect("excerpt context");
    assert!(excerpt_context
        .source_excerpts
        .iter()
        .all(|excerpt| { excerpt.end_line.saturating_sub(excerpt.start_line) < 2 }));
}

#[test]
fn semantic_tags_and_database_edges_remain_neutral_and_consistent() {
    let root = copy_fixture("bytecode_full_mode");
    let report = SuiMoveIndexer::new(IndexerConfig::default())
        .index_package(root.path(), "bytecode-run".to_string())
        .expect("index package");
    let db_path = root.path().join(".peregrine/index.sqlite");
    let connection = Connection::open(db_path).expect("db");

    for prohibited in [
        "vulnerable",
        "safe",
        "unguarded_transfer",
        "missing_authorization",
        "auth_bypass",
        "exploitable",
    ] {
        let count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM semantic_tags WHERE package_id = ?1 AND tag LIKE ?2",
                (&report.package_id, format!("%{prohibited}%")),
                |row| row.get(0),
            )
            .expect("prohibited tag count");
        assert_eq!(count, 0);
    }

    let dangling_operation_edges: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM edges LEFT JOIN operations ON operations.id = edges.operation_id WHERE edges.package_id = ?1 AND edges.operation_id IS NOT NULL AND operations.id IS NULL",
            [&report.package_id],
            |row| row.get(0),
        )
        .expect("dangling operation edges");
    assert_eq!(dangling_operation_edges, 0);

    let dangling_tags: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM semantic_tags
             LEFT JOIN functions ON functions.id = semantic_tags.target_id
             LEFT JOIN types ON types.id = semantic_tags.target_id
             LEFT JOIN operations ON operations.id = semantic_tags.target_id
             WHERE semantic_tags.package_id = ?1
               AND functions.id IS NULL
               AND types.id IS NULL
               AND operations.id IS NULL",
            [&report.package_id],
            |row| row.get(0),
        )
        .expect("dangling tags");
    assert_eq!(dangling_tags, 0);
}

#[test]
fn all_required_sui_fixtures_are_present() {
    for fixture in REQUIRED_FIXTURES {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/sui")
            .join(fixture);
        assert!(path.is_dir(), "missing fixture {fixture}");
    }
}

#[test]
fn simple_module_fixture_indexes_struct_fields_and_functions() {
    let fixture = index_fixture("simple_module");
    let connection = Connection::open(&fixture.db_path).expect("db");
    assert_eq!(fixture.report.module_count, 1);
    assert!(fixture.report.type_count >= 1);
    assert!(fixture.report.function_count >= 2);

    let fields: i64 = connection
        .query_row("SELECT COUNT(*) FROM fields", [], |row| row.get(0))
        .expect("field count");
    assert_eq!(fields, 2);
    let value = find_function(&fixture.db_path, &fixture.report.package_id, "value");
    let context = fixture
        .indexer
        .get_function_context(&fixture.db_path, &value.id, &ContextBudget::default())
        .expect("context");
    assert!(context
        .outline
        .params
        .iter()
        .any(|param| param.type_name == "&simple_module::main::Counter"));
}

#[test]
fn entry_function_fixture_indexes_entry_and_mutable_reference_tags() {
    let fixture = index_fixture("entry_function");
    let entries = fixture
        .indexer
        .get_public_entry_functions(&fixture.db_path, &fixture.report.package_id)
        .expect("entry functions");
    assert_eq!(entries.len(), 1);
    assert!(entries[0].full_name.ends_with("increment"));

    let context = fixture
        .indexer
        .get_function_context(
            &fixture.db_path,
            &entries[0].id,
            &ContextBudget {
                max_tokens_estimate: 1_500,
                ..ContextBudget::default()
            },
        )
        .expect("entry context");
    assert!(context
        .card
        .top_tags
        .contains(&"public_entry_detected".to_string()));
    assert!(context
        .card
        .top_tags
        .contains(&"mutable_reference_parameter_detected".to_string()));
    assert!(context
        .card
        .top_tags
        .contains(&"tx_context_parameter_detected".to_string()));
}

#[test]
fn struct_abilities_fixture_indexes_abilities_and_key_object_tags() {
    let fixture = index_fixture("struct_abilities");
    let connection = Connection::open(&fixture.db_path).expect("db");
    let key_object_tags: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM semantic_tags WHERE tag = 'key_object_type_detected'",
            [],
            |row| row.get(0),
        )
        .expect("key object tags");
    assert_eq!(key_object_tags, 1);
    let store_tags: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM semantic_tags WHERE tag = 'store_type_detected'",
            [],
            |row| row.get(0),
        )
        .expect("store tags");
    assert!(store_tags >= 2);
}

#[test]
fn field_read_write_fixture_extracts_operations_and_edges() {
    let fixture = index_fixture("field_read_write");
    let add = find_function(&fixture.db_path, &fixture.report.package_id, "add");
    let budget = ContextBudget {
        level: ContextLevel::Level2,
        max_operations: 128,
        max_tokens_estimate: 2_000,
        ..ContextBudget::default()
    };
    let operations = fixture
        .indexer
        .get_function_operations(&fixture.db_path, &add.id, &budget)
        .expect("operations");
    assert!(operations
        .iter()
        .any(|op| op.kind == OperationKind::ReadField));
    assert!(operations
        .iter()
        .any(|op| op.kind == OperationKind::WriteField));
    assert!(!fixture
        .indexer
        .get_function_field_reads(&fixture.db_path, &add.id)
        .expect("field reads")
        .is_empty());
    assert!(!fixture
        .indexer
        .get_function_field_writes(&fixture.db_path, &add.id)
        .expect("field writes")
        .is_empty());
}

#[test]
fn assert_then_call_fixture_orders_assert_before_transfer_call() {
    let fixture = index_fixture("assert_then_call");
    let function = find_function(
        &fixture.db_path,
        &fixture.report.package_id,
        "check_then_send",
    );
    let operations = fixture
        .indexer
        .get_function_operations(
            &fixture.db_path,
            &function.id,
            &ContextBudget {
                max_operations: 128,
                ..ContextBudget::default()
            },
        )
        .expect("operations");
    let assert_index = operation_position(&operations, OperationKind::Assert);
    let transfer_index = operations
        .iter()
        .position(is_transfer_operation)
        .expect("transfer operation");
    assert!(assert_index < transfer_index);
}

#[test]
fn transfer_call_direct_fixture_tags_transfer_without_findings() {
    let fixture = index_fixture("transfer_call_direct");
    let transfer_ops = fixture
        .indexer
        .get_operations_by_tag(
            &fixture.db_path,
            &fixture.report.package_id,
            "transfer_api_call_detected",
            &ContextBudget::default(),
        )
        .expect("transfer tag");
    assert_eq!(transfer_ops.len(), 1);
    assert_neutral_db(&fixture.db_path, &fixture.report.package_id);
}

#[test]
fn transfer_call_wrapper_fixture_reaches_inner_transfer_call() {
    let fixture = index_fixture("transfer_call_wrapper");
    let send = find_function(&fixture.db_path, &fixture.report.package_id, "send");
    let send_inner = find_function(&fixture.db_path, &fixture.report.package_id, "send_inner");
    let reachable = fixture
        .indexer
        .get_reachable_callees(
            &fixture.db_path,
            &send.id,
            2,
            &ContextBudget {
                max_call_depth: 2,
                max_callees: 8,
                ..ContextBudget::default()
            },
        )
        .expect("reachable");
    assert!(reachable.contains(&send_inner.id));
    assert!(reachable
        .iter()
        .any(|callee| callee.contains("::transfer::public_transfer")));
}

#[test]
fn transfer_then_assert_fixture_preserves_operation_order() {
    let fixture = index_fixture("transfer_then_assert");
    let function = find_function(
        &fixture.db_path,
        &fixture.report.package_id,
        "transfer_then_check",
    );
    let operations = fixture
        .indexer
        .get_function_operations(
            &fixture.db_path,
            &function.id,
            &ContextBudget {
                max_operations: 128,
                ..ContextBudget::default()
            },
        )
        .expect("operations");
    let transfer_index = operations
        .iter()
        .position(is_transfer_operation)
        .expect("transfer operation");
    let assert_index = operation_position(&operations, OperationKind::Assert);
    assert!(transfer_index < assert_index);
    assert_neutral_db(&fixture.db_path, &fixture.report.package_id);
}

#[test]
fn dynamic_field_usage_fixture_tags_dynamic_field_calls() {
    let fixture = index_fixture("dynamic_field_usage");
    let operations = fixture
        .indexer
        .get_operations_by_tag(
            &fixture.db_path,
            &fixture.report.package_id,
            "dynamic_field_api_call_detected",
            &ContextBudget {
                max_operations: 16,
                ..ContextBudget::default()
            },
        )
        .expect("dynamic field tags");
    assert!(operations.len() >= 2);
}

#[test]
fn clock_usage_fixture_indexes_clock_type_and_api_tag() {
    let fixture = index_fixture("clock_usage");
    let now = find_function(&fixture.db_path, &fixture.report.package_id, "now_ms");
    let context = fixture
        .indexer
        .get_function_context(&fixture.db_path, &now.id, &ContextBudget::default())
        .expect("context");
    assert!(context
        .outline
        .params
        .iter()
        .any(|param| param.type_name == "&sui::clock::Clock"));
    let clock_ops = fixture
        .indexer
        .get_operations_by_tag(
            &fixture.db_path,
            &fixture.report.package_id,
            "clock_api_call_detected",
            &ContextBudget::default(),
        )
        .expect("clock tags");
    assert_eq!(clock_ops.len(), 1);
}

#[test]
fn tx_context_usage_fixture_tags_parameter_and_call() {
    let fixture = index_fixture("tx_context_usage");
    let sender = find_function(&fixture.db_path, &fixture.report.package_id, "sender");
    let context = fixture
        .indexer
        .get_function_context(&fixture.db_path, &sender.id, &ContextBudget::default())
        .expect("context");
    assert!(context
        .card
        .top_tags
        .contains(&"tx_context_parameter_detected".to_string()));
    let tx_ops = fixture
        .indexer
        .get_operations_by_tag(
            &fixture.db_path,
            &fixture.report.package_id,
            "tx_context_api_call_detected",
            &ContextBudget::default(),
        )
        .expect("tx context ops");
    assert_eq!(tx_ops.len(), 1);
}

#[test]
fn generic_function_fixture_indexes_type_params_and_constraints() {
    let fixture = index_fixture("generic_function");
    let wrap = find_function(&fixture.db_path, &fixture.report.package_id, "wrap");
    let context = fixture
        .indexer
        .get_function_context(&fixture.db_path, &wrap.id, &ContextBudget::default())
        .expect("context");
    assert!(context
        .card
        .top_tags
        .contains(&"generic_function_detected".to_string()));
    let function = Connection::open(&fixture.db_path)
        .expect("db")
        .query_row(
            "SELECT type_parameters_json FROM functions WHERE id = ?1",
            [&wrap.id],
            |row| row.get::<_, String>(0),
        )
        .expect("type params");
    assert!(function.contains("T: store"));
}

#[test]
fn friend_function_fixture_indexes_friend_visibility_and_edge() {
    let fixture = index_fixture("friend_function");
    let friend_only = find_function(&fixture.db_path, &fixture.report.package_id, "friend_only");
    let context = fixture
        .indexer
        .get_function_context(&fixture.db_path, &friend_only.id, &ContextBudget::default())
        .expect("context");
    assert_eq!(context.card.visibility, "PublicFriend");
    assert!(context
        .card
        .top_tags
        .contains(&"friend_function_detected".to_string()));
    let friend_edges: i64 = Connection::open(&fixture.db_path)
        .expect("db")
        .query_row(
            "SELECT COUNT(*) FROM edges WHERE edge_type = 'Friends'",
            [],
            |row| row.get(0),
        )
        .expect("friend edges");
    assert_eq!(friend_edges, 1);
}

#[test]
fn broken_package_fixture_reports_diagnostics_without_fake_full_index() {
    let fixture = index_fixture("broken_package");
    assert_eq!(fixture.report.module_count, 0);
    assert_eq!(fixture.report.operation_count, 0);
    assert!(fixture.report.diagnostic_count >= 1);
    assert!(matches!(
        fixture.report.status.as_str(),
        "PartialWithDiagnostics" | "FailedToCompile"
    ));
}

#[test]
fn operation_order_complex_fixture_extracts_blocks_branch_abort_and_add() {
    let fixture = index_fixture("operation_order_complex");
    let branchy = find_function(&fixture.db_path, &fixture.report.package_id, "branchy");
    let operations = fixture
        .indexer
        .get_function_operations(
            &fixture.db_path,
            &branchy.id,
            &ContextBudget {
                max_operations: 128,
                ..ContextBudget::default()
            },
        )
        .expect("operations");
    assert!(operations
        .iter()
        .any(|op| op.kind == OperationKind::BranchIf));
    assert!(operations.iter().any(|op| op.kind == OperationKind::Abort));
    assert!(operations.iter().any(|op| op.kind == OperationKind::Add));
    let connection = Connection::open(&fixture.db_path).expect("db");
    let blocks: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM basic_blocks WHERE function_id = ?1",
            [&branchy.id],
            |row| row.get(0),
        )
        .expect("blocks");
    let control_flow_edges: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM edges WHERE edge_type = 'ControlFlow'",
            [],
            |row| row.get(0),
        )
        .expect("control flow");
    assert!(blocks > 1);
    assert!(control_flow_edges > 0);
}

#[test]
fn pack_unpack_fixture_extracts_pack_and_unpack_operations() {
    let fixture = index_fixture("pack_unpack");
    let make = find_function(&fixture.db_path, &fixture.report.package_id, "make");
    let destroy = find_function(&fixture.db_path, &fixture.report.package_id, "destroy");
    let budget = ContextBudget {
        max_operations: 128,
        ..ContextBudget::default()
    };
    assert!(fixture
        .indexer
        .get_function_operations(&fixture.db_path, &make.id, &budget)
        .expect("make ops")
        .iter()
        .any(|op| op.kind == OperationKind::Pack));
    assert!(fixture
        .indexer
        .get_function_operations(&fixture.db_path, &destroy.id, &budget)
        .expect("destroy ops")
        .iter()
        .any(|op| op.kind == OperationKind::Unpack));
}

#[test]
fn model_context_pack_fixture_contains_compact_function_evidence() {
    let fixture = index_fixture("model_context_pack");
    let deposit = find_function(&fixture.db_path, &fixture.report.package_id, "deposit");
    let pack = fixture
        .indexer
        .get_context_pack(
            &fixture.db_path,
            &deposit.id,
            &ContextBudget {
                level: ContextLevel::Level2,
                include_source: true,
                max_tokens_estimate: 2_500,
                max_operations: 128,
                ..ContextBudget::default()
            },
        )
        .expect("context pack");
    let serialized = serde_json::to_string(&pack).expect("json");
    assert!(pack.estimated_tokens <= pack.budget_tokens);
    assert!(serialized.contains("deposit"));
    assert!(serialized.contains("Assert"));
    assert!(serialized.contains("Call"));
    assert!(!serialized.contains("debug_raw_summary_json"));
}

#[test]
fn large_function_budget_fixture_trims_context_and_keeps_assert() {
    let fixture = index_fixture("large_function_budget");
    let large = find_function(&fixture.db_path, &fixture.report.package_id, "large");
    let context = fixture
        .indexer
        .get_function_context(
            &fixture.db_path,
            &large.id,
            &ContextBudget {
                level: ContextLevel::Level2,
                max_tokens_estimate: 900,
                max_operations: 256,
                ..ContextBudget::default()
            },
        )
        .expect("context");
    assert!(context.trimmed);
    assert!(context.estimated_tokens <= context.budget_tokens);
    assert!(context
        .operations
        .iter()
        .any(|op| op.kind == OperationKind::Assert));
}

#[test]
fn duplicate_type_dedup_fixture_returns_related_type_once() {
    let fixture = index_fixture("duplicate_type_dedup");
    let combine = find_function(&fixture.db_path, &fixture.report.package_id, "combine");
    let context = fixture
        .indexer
        .get_function_context(
            &fixture.db_path,
            &combine.id,
            &ContextBudget {
                max_related_types: 8,
                max_tokens_estimate: 2_000,
                ..ContextBudget::default()
            },
        )
        .expect("context");
    let shared_count = context
        .related_types
        .iter()
        .filter(|type_card| type_card.full_name.ends_with("Shared"))
        .count();
    assert_eq!(shared_count, 1);
}

#[test]
fn progressive_context_levels_fixture_expands_monotonically() {
    let fixture = index_fixture("progressive_context_levels");
    let entry = find_function(&fixture.db_path, &fixture.report.package_id, "entry");
    let level0 = fixture
        .indexer
        .get_function_context(
            &fixture.db_path,
            &entry.id,
            &ContextBudget {
                level: ContextLevel::Level0,
                max_tokens_estimate: 1_000,
                ..ContextBudget::default()
            },
        )
        .expect("level0");
    let level1 = fixture
        .indexer
        .get_function_context(
            &fixture.db_path,
            &entry.id,
            &ContextBudget {
                level: ContextLevel::Level1,
                max_tokens_estimate: 1_500,
                ..ContextBudget::default()
            },
        )
        .expect("level1");
    let level2 = fixture
        .indexer
        .get_function_context(
            &fixture.db_path,
            &entry.id,
            &ContextBudget {
                level: ContextLevel::Level2,
                max_tokens_estimate: 2_000,
                max_operations: 64,
                ..ContextBudget::default()
            },
        )
        .expect("level2");
    let level3 = fixture
        .indexer
        .get_function_context(
            &fixture.db_path,
            &entry.id,
            &ContextBudget {
                level: ContextLevel::Level3,
                max_tokens_estimate: 3_000,
                max_call_depth: 3,
                max_callees: 8,
                ..ContextBudget::default()
            },
        )
        .expect("level3");
    assert!(level0.operations.is_empty());
    assert!(level1.operations.is_empty());
    assert!(!level2.operations.is_empty());
    assert!(!level3.reachable_callees.is_empty());
    assert!(level1.estimated_tokens >= level0.estimated_tokens);
    assert!(level2.estimated_tokens >= level1.estimated_tokens);
}

#[test]
fn source_excerpt_budget_fixture_bounds_excerpts_and_requires_explicit_full_source() {
    let fixture = index_fixture("source_excerpt_budget");
    let function = find_function(&fixture.db_path, &fixture.report.package_id, "long_source");
    let bounded = fixture
        .indexer
        .get_function_context(
            &fixture.db_path,
            &function.id,
            &ContextBudget {
                level: ContextLevel::Level2,
                include_source: true,
                max_source_excerpt_lines: 3,
                max_tokens_estimate: 2_000,
                ..ContextBudget::default()
            },
        )
        .expect("bounded context");
    assert!(bounded
        .source_excerpts
        .iter()
        .all(|excerpt| excerpt.end_line.saturating_sub(excerpt.start_line) < 3));
    let without_source = fixture
        .indexer
        .get_function_context(
            &fixture.db_path,
            &function.id,
            &ContextBudget {
                level: ContextLevel::Level2,
                include_source: false,
                max_tokens_estimate: 2_000,
                ..ContextBudget::default()
            },
        )
        .expect("without source");
    assert!(without_source.source_excerpts.is_empty());
}

#[test]
fn call_graph_budget_fixture_enforces_depth_and_callee_limits() {
    let fixture = index_fixture("call_graph_budget");
    let entry = find_function(&fixture.db_path, &fixture.report.package_id, "entry");
    let shallow = fixture
        .indexer
        .get_reachable_callees(
            &fixture.db_path,
            &entry.id,
            5,
            &ContextBudget {
                max_call_depth: 2,
                max_callees: 8,
                ..ContextBudget::default()
            },
        )
        .expect("shallow graph");
    assert_eq!(shallow.len(), 2);
    let limited = fixture
        .indexer
        .get_reachable_callees(
            &fixture.db_path,
            &entry.id,
            5,
            &ContextBudget {
                max_call_depth: 5,
                max_callees: 3,
                ..ContextBudget::default()
            },
        )
        .expect("limited graph");
    assert_eq!(limited.len(), 3);
    let graph = fixture
        .indexer
        .get_call_graph(
            &fixture.db_path,
            &entry.id,
            5,
            &ContextBudget {
                max_call_depth: 2,
                max_callees: 8,
                ..ContextBudget::default()
            },
        )
        .expect("graph view");
    assert!(graph.trimmed);
    assert!(graph
        .trim_reasons
        .contains(&"call_depth_limited".to_string()));
}

const REQUIRED_FIXTURES: &[&str] = &[
    "package_summaries_pointer_basic",
    "package_summaries_lazy_materialization",
    "package_summaries_malformed",
    "simple_module",
    "entry_function",
    "struct_abilities",
    "field_read_write",
    "assert_then_call",
    "transfer_call_direct",
    "transfer_call_wrapper",
    "transfer_then_assert",
    "dynamic_field_usage",
    "clock_usage",
    "tx_context_usage",
    "generic_function",
    "friend_function",
    "broken_package",
    "operation_order_complex",
    "pack_unpack",
    "model_context_pack",
    "large_function_budget",
    "duplicate_type_dedup",
    "progressive_context_levels",
    "source_excerpt_budget",
    "call_graph_budget",
];

struct IndexedFixture {
    _root: tempfile::TempDir,
    indexer: SuiMoveIndexer,
    report: IndexReport,
    db_path: PathBuf,
}

fn index_fixture(name: &str) -> IndexedFixture {
    let root = copy_fixture(name);
    let indexer = SuiMoveIndexer::new(IndexerConfig::default());
    let report = indexer
        .index_package(root.path(), format!("{name}-run"))
        .expect("index package");
    let db_path = root.path().join(".peregrine/index.sqlite");
    IndexedFixture {
        _root: root,
        indexer,
        report,
        db_path,
    }
}

fn copy_fixture(name: &str) -> tempfile::TempDir {
    let temp = tempdir().expect("tempdir");
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sui")
        .join(name);
    copy_dir(&source, temp.path());
    temp
}

fn find_function(
    db_path: &Path,
    package_id: &str,
    query: &str,
) -> peregrine_indexer::sui::model::SymbolResult {
    let reader = SqliteIndexReader::open(db_path).expect("reader");
    reader
        .search_symbols(package_id, query, &ContextBudget::default())
        .expect("search")
        .into_iter()
        .find(|symbol| symbol.kind == "function" && symbol.full_name.ends_with(query))
        .expect("searched function was not indexed")
}

fn operation_position(
    operations: &[peregrine_indexer::core::Operation],
    kind: OperationKind,
) -> usize {
    operations
        .iter()
        .position(|operation| operation.kind == kind)
        .expect("operation kind was not indexed")
}

fn is_transfer_operation(operation: &peregrine_indexer::core::Operation) -> bool {
    operation.kind == OperationKind::Call
        && operation
            .target
            .as_deref()
            .is_some_and(|target| target.contains("::transfer::public_transfer"))
}

fn assert_neutral_db(db_path: &Path, package_id: &str) {
    let connection = Connection::open(db_path).expect("db");
    for prohibited in [
        "vulnerable",
        "safe",
        "unguarded_transfer",
        "missing_authorization",
        "auth_bypass",
        "exploitable",
        "guaranteed_guarded",
    ] {
        let count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM semantic_tags WHERE package_id = ?1 AND tag LIKE ?2",
                (package_id, format!("%{prohibited}%")),
                |row| row.get(0),
            )
            .expect("neutral tag count");
        assert_eq!(count, 0);
    }
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
