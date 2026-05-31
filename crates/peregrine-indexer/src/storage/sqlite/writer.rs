use std::path::Path;

use rusqlite::{Connection, params};

use crate::{
    core::{FieldInfo, TypeDef},
    storage::sqlite::migrations::migrate,
    sui::model::{DependencyRecord, ProgramIndex, SourceFileRecord},
};

pub struct SqliteIndexWriter {
    connection: Connection,
}

impl SqliteIndexWriter {
    pub fn open(path: &Path) -> rusqlite::Result<Self> {
        let connection = Connection::open(path)?;
        migrate(&connection)?;
        Ok(Self { connection })
    }

    pub fn write_program_index(&mut self, index: &ProgramIndex) -> rusqlite::Result<()> {
        let tx = self.connection.transaction()?;
        tx.execute(
            "DELETE FROM packages WHERE id = ?1",
            [index.package.id.as_str()],
        )?;

        tx.execute(
            "INSERT INTO packages (id, name, root_path, manifest_path, role, compiler_version, package_hash, status, indexed_at, metadata_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                index.package.id,
                index.package.name,
                index.package.root_path,
                index.package.manifest_path,
                enum_name(&index.package.role),
                index.package.compiler_version,
                index.package.package_hash,
                enum_name(&index.package.status),
                index.package.indexed_at,
                optional_json(&index.package.metadata_json)?,
            ],
        )?;

        for file in &index.files {
            insert_file(&tx, &index.package.id, file)?;
        }
        for artifact in &index.summary_artifacts {
            tx.execute(
                "INSERT INTO summary_artifacts (id, package_id, package_alias, module_name, summary_path, content_hash, schema_version, role, materialized_status, last_seen_at, card_json, raw_summary_json)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, NULL)",
                params![
                    artifact.id,
                    artifact.package_id,
                    artifact.package_alias,
                    artifact.module_name,
                    artifact.summary_path,
                    artifact.content_hash,
                    artifact.schema_version,
                    enum_name(&artifact.role),
                    enum_name(&artifact.materialized_status),
                    artifact.last_seen_at,
                    optional_json(&artifact.card_json)?,
                ],
            )?;
        }
        for mapping in &index.address_mappings {
            tx.execute(
                "INSERT INTO address_mappings (id, package_id, alias, address) VALUES (?1, ?2, ?3, ?4)",
                params![mapping.id, mapping.package_id, mapping.alias, mapping.address],
            )?;
        }
        for module in &index.modules {
            tx.execute(
                "INSERT INTO modules (id, package_id, summary_artifact_id, file_id, address, name, full_name, immediate_dependencies_json, docs, attributes_json, source_span_json)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                params![
                    module.id,
                    module.package_id,
                    module.summary_artifact_id,
                    module.file_id,
                    module.address,
                    module.name,
                    module.full_name,
                    json(&module.immediate_dependencies)?,
                    module.docs,
                    json(&module.attributes)?,
                    json(&module.source_span)?,
                ],
            )?;
        }
        for dependency in &index.dependencies {
            insert_dependency(&tx, dependency)?;
        }
        for type_def in &index.types {
            insert_type(&tx, type_def)?;
            for field in &type_def.fields {
                insert_field(&tx, field)?;
            }
        }
        for field in &index.fields {
            insert_field(&tx, field)?;
        }
        for function in &index.functions {
            tx.execute(
                "INSERT INTO functions (id, package_id, module_id, name, full_name, visibility, is_entry, is_native, type_parameters_json, parameters_json, returns_json, acquires_json, docs, attributes_json, source_span_json)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
                params![
                    function.id,
                    function.package_id,
                    function.module_id,
                    function.name,
                    function.full_name,
                    enum_name(&function.visibility),
                    function.is_entry as i64,
                    function.is_native as i64,
                    json(&function.type_parameters)?,
                    json(&function.parameters)?,
                    json(&function.returns)?,
                    json(&function.acquires)?,
                    function.docs,
                    json(&function.attributes)?,
                    json(&function.source_span)?,
                ],
            )?;
        }
        for local in &index.locals {
            tx.execute(
                "INSERT INTO locals (id, package_id, function_id, name, type_name, index_in_function, source_span_json)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    local.id,
                    local.package_id,
                    local.function_id,
                    local.name,
                    local.type_name,
                    local.index_in_function.map(|index| index as i64),
                    json(&local.source_span)?,
                ],
            )?;
        }
        for block in &index.basic_blocks {
            tx.execute(
                "INSERT INTO basic_blocks (id, package_id, function_id, index_in_function, label, start_operation_index, end_operation_index, source_span_json)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    block.id,
                    block.package_id,
                    block.function_id,
                    block.index_in_function as i64,
                    block.label,
                    block.start_operation_index.map(|index| index as i64),
                    block.end_operation_index.map(|index| index as i64),
                    json(&block.source_span)?,
                ],
            )?;
        }
        for operation in &index.operations {
            tx.execute(
                "INSERT INTO operations (id, package_id, function_id, index_in_function, kind, display, target, source_span_json, metadata_json)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    operation.id,
                    operation.package_id,
                    operation.function_id,
                    operation.index_in_function as i64,
                    enum_name(&operation.kind),
                    operation.display,
                    operation.target,
                    json(&operation.source_span)?,
                    optional_json(&operation.metadata_json)?,
                ],
            )?;
        }
        for edge in &index.edges {
            tx.execute(
                "INSERT INTO edges (id, package_id, from_id, to_id, edge_type, operation_id, source_span_json, metadata_json)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    edge.id,
                    edge.package_id,
                    edge.from_id,
                    edge.to_id,
                    enum_name(&edge.edge_type),
                    edge.operation_id,
                    json(&edge.source_span)?,
                    optional_json(&edge.metadata_json)?,
                ],
            )?;
        }
        for tag in &index.semantic_tags {
            tx.execute(
                "INSERT INTO semantic_tags (id, package_id, target_id, tag, source_span_json, metadata_json)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    tag.id,
                    tag.package_id,
                    tag.target_id,
                    tag.tag,
                    json(&tag.source_span)?,
                    optional_json(&tag.metadata_json)?,
                ],
            )?;
        }
        for diagnostic in &index.diagnostics {
            tx.execute(
                "INSERT INTO diagnostics (id, package_id, severity, source, message, source_span_json, metadata_json)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    diagnostic.id,
                    diagnostic.package_id,
                    enum_name(&diagnostic.severity),
                    diagnostic.source,
                    diagnostic.message,
                    json(&diagnostic.source_span)?,
                    optional_json(&diagnostic.metadata_json)?,
                ],
            )?;
        }

        tx.commit()
    }

    pub fn update_summary_card(
        &self,
        artifact_id: &str,
        materialized_status: &str,
        card_json: &serde_json::Value,
    ) -> rusqlite::Result<()> {
        self.connection.execute(
            "UPDATE summary_artifacts SET materialized_status = ?1, card_json = ?2 WHERE id = ?3",
            params![materialized_status, json(card_json)?, artifact_id],
        )?;
        Ok(())
    }
}

fn insert_file(
    tx: &rusqlite::Transaction<'_>,
    package_id: &str,
    file: &SourceFileRecord,
) -> rusqlite::Result<()> {
    tx.execute(
        "INSERT INTO files (id, package_id, path, content_hash, kind) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![file.id, package_id, file.path, file.content_hash, file.kind],
    )?;
    Ok(())
}

fn insert_dependency(
    tx: &rusqlite::Transaction<'_>,
    dependency: &DependencyRecord,
) -> rusqlite::Result<()> {
    tx.execute(
        "INSERT INTO dependencies (id, package_id, source_package_alias, source_module, target_package_alias, target_module, dependency_kind, metadata_json)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            dependency.id,
            dependency.package_id,
            dependency.source_package_alias,
            dependency.source_module,
            dependency.target_package_alias,
            dependency.target_module,
            dependency.dependency_kind,
            optional_json(&dependency.metadata_json)?,
        ],
    )?;
    Ok(())
}

fn insert_type(tx: &rusqlite::Transaction<'_>, type_def: &TypeDef) -> rusqlite::Result<()> {
    tx.execute(
        "INSERT INTO types (id, package_id, module_id, name, full_name, kind, abilities_json, type_parameters_json, docs, attributes_json, source_span_json)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            type_def.id,
            type_def.package_id,
            type_def.module_id,
            type_def.name,
            type_def.full_name,
            enum_name(&type_def.kind),
            json(&type_def.abilities)?,
            json(&type_def.type_parameters)?,
            type_def.docs,
            json(&type_def.attributes)?,
            json(&type_def.source_span)?,
        ],
    )?;
    Ok(())
}

fn insert_field(tx: &rusqlite::Transaction<'_>, field: &FieldInfo) -> rusqlite::Result<()> {
    tx.execute(
        "INSERT OR IGNORE INTO fields (id, package_id, module_id, type_id, name, type_name, source_span_json)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            field.id,
            field.package_id,
            field.module_id,
            field.type_id,
            field.name,
            field.type_name,
            json(&field.source_span)?,
        ],
    )?;
    Ok(())
}

fn json<T: serde::Serialize>(value: &T) -> rusqlite::Result<String> {
    serde_json::to_string(value)
        .map_err(|error| rusqlite::Error::ToSqlConversionFailure(error.into()))
}

fn optional_json(value: &Option<serde_json::Value>) -> rusqlite::Result<Option<String>> {
    value.as_ref().map(json).transpose()
}

fn enum_name<T: std::fmt::Debug>(value: &T) -> String {
    format!("{value:?}")
}
