use std::{fs, path::Path};

use rusqlite::{params, Connection, OptionalExtension};

use crate::{
    core::{
        estimate_tokens, ContextBudget, Diagnostic, FunctionInfo, FunctionParameter,
        FunctionVisibility, ModuleInfo, Operation, OperationKind, SemanticTag, SourceSpan, TypeDef,
        TypeKind,
    },
    sui::model::{
        ContextPack, FunctionContext, FunctionEvidenceSummary, FunctionOutline, FunctionSymbolCard,
        GraphView, ModuleContext, ModuleSummaryCard, OperationHistogramEntry, PackageOverview,
        RelatedTypeCard, SourceExcerpt, SymbolResult, TypeContext,
    },
};

use super::migrations::migrate;

pub struct SqliteIndexReader {
    connection: Connection,
}

impl SqliteIndexReader {
    pub fn open(path: &Path) -> rusqlite::Result<Self> {
        let connection = Connection::open(path)?;
        migrate(&connection)?;
        Ok(Self { connection })
    }

    pub fn package_root_path(&self, package_id: &str) -> rusqlite::Result<Option<String>> {
        self.connection
            .query_row(
                "SELECT root_path FROM packages WHERE id = ?1",
                [package_id],
                |row| row.get(0),
            )
            .optional()
    }

    pub fn latest_db_package_id(&self) -> rusqlite::Result<Option<String>> {
        self.connection
            .query_row(
                "SELECT id FROM packages ORDER BY indexed_at DESC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .optional()
    }

    pub fn get_package_overview(&self, package_id: &str) -> rusqlite::Result<PackageOverview> {
        let (id, name, root_path, status, indexed_at, metadata_json): (
            String,
            String,
            String,
            String,
            i64,
            Option<serde_json::Value>,
        ) = self.connection.query_row(
                "SELECT id, name, root_path, status, indexed_at, metadata_json FROM packages WHERE id = ?1",
                [package_id],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        optional_json_from_col(row, 5)?,
                    ))
                },
            )?;
        let modules = count(&self.connection, "modules", package_id)?;
        let functions = count(&self.connection, "functions", package_id)?;
        let types = count(&self.connection, "types", package_id)?;
        let summary_artifacts = count(&self.connection, "summary_artifacts", package_id)?;
        let pointer_only_summaries = self.connection.query_row(
            "SELECT COUNT(*) FROM summary_artifacts WHERE package_id = ?1 AND materialized_status = 'PointerOnly'",
            [package_id],
            |row| row.get::<_, i64>(0),
        )? as usize;

        Ok(PackageOverview {
            id,
            name,
            root_path,
            status,
            indexed_at,
            index_health: metadata_json
                .as_ref()
                .and_then(|metadata| metadata.get("index_health"))
                .cloned(),
            modules,
            functions,
            types,
            summary_artifacts,
            pointer_only_summaries,
        })
    }

    pub fn get_module_context(&self, module_id: &str) -> rusqlite::Result<ModuleContext> {
        let module = self.load_module(module_id)?;
        let functions = self.symbols_for_module(module_id, "function")?;
        let types = self.symbols_for_module(module_id, "type")?;

        Ok(ModuleContext {
            module,
            functions,
            types,
        })
    }

    pub fn get_type_context(&self, type_id: &str) -> rusqlite::Result<TypeContext> {
        let type_def = self.load_type(type_id)?;
        Ok(TypeContext { type_def })
    }

    pub fn get_function_context(
        &self,
        function_id: &str,
        budget: &ContextBudget,
    ) -> rusqlite::Result<FunctionContext> {
        let function = self.load_function(function_id)?;
        let tags = if budget.include_semantic_tags {
            self.tags_for_target(function_id)?
        } else {
            Vec::new()
        };
        let operations = if budget.level >= crate::core::ContextLevel::Level2 {
            self.operations_for_function(function_id, budget.max_operations)?
        } else {
            Vec::new()
        };
        let callees = if budget.include_callees {
            self.direct_calls(function_id, budget.max_callees)?
        } else {
            Vec::new()
        };
        let callers = if budget.include_callers {
            self.get_function_callers(function_id, budget)?
        } else {
            Vec::new()
        };
        let reachable_callees = if budget.include_reachable_graph
            || budget.level >= crate::core::ContextLevel::Level3
        {
            self.get_reachable_callees(function_id, budget.max_call_depth, budget)?
        } else {
            Vec::new()
        };
        let field_reads = self.get_function_field_reads(function_id)?;
        let field_writes = self.get_function_field_writes(function_id)?;
        let related_types = if budget.include_related_types {
            self.related_types_for_function(&function, budget.max_related_types)?
        } else {
            Vec::new()
        };
        let operation_count = self.operation_count(function_id)?;
        let operation_histogram = self.operation_histogram(function_id)?;
        let source_excerpts = if budget.include_source || budget.include_full_source {
            self.source_excerpts_for_function(&function, budget)?
        } else {
            Vec::new()
        };
        let evidence = self.function_evidence_summary(
            &function,
            operation_count,
            field_reads.len(),
            field_writes.len(),
        )?;
        let outline = FunctionOutline {
            params: function.parameters.clone(),
            returns: function.returns.clone(),
            direct_calls: callees.clone(),
            operation_count,
            tags: tags.iter().map(|tag| tag.tag.clone()).collect(),
        };
        let mut context = FunctionContext {
            card: FunctionSymbolCard::from_function(&function, tags),
            outline,
            evidence,
            callers,
            callees,
            reachable_callees,
            field_reads,
            field_writes,
            related_types,
            operation_histogram,
            operations,
            source_excerpts,
            diagnostics: if budget.include_diagnostics {
                self.diagnostics_for_package(&function.package_id)?
            } else {
                Vec::new()
            },
            estimated_tokens: 0,
            budget_tokens: budget.max_tokens_estimate,
            trimmed: false,
            trim_reasons: Vec::new(),
        };
        apply_function_budget(&mut context);
        Ok(context)
    }

    pub fn get_function_operations(
        &self,
        function_id: &str,
        budget: &ContextBudget,
    ) -> rusqlite::Result<Vec<Operation>> {
        self.operations_for_function(function_id, budget.max_operations)
    }

    pub fn get_function_body(
        &self,
        function_id: &str,
        budget: &ContextBudget,
    ) -> rusqlite::Result<FunctionContext> {
        let mut body_budget = budget.clone();
        if body_budget.level < crate::core::ContextLevel::Level2 {
            body_budget.level = crate::core::ContextLevel::Level2;
        }
        self.get_function_context(function_id, &body_budget)
    }

    pub fn get_function_callers(
        &self,
        function_id: &str,
        budget: &ContextBudget,
    ) -> rusqlite::Result<Vec<String>> {
        self.edge_neighbors("to_id", "from_id", function_id, budget.max_callers)
    }

    pub fn get_function_callees(
        &self,
        function_id: &str,
        budget: &ContextBudget,
    ) -> rusqlite::Result<Vec<String>> {
        self.edge_neighbors("from_id", "to_id", function_id, budget.max_callees)
    }

    pub fn get_reachable_callees(
        &self,
        function_id: &str,
        depth: usize,
        budget: &ContextBudget,
    ) -> rusqlite::Result<Vec<String>> {
        let mut seen = vec![function_id.to_string()];
        let mut reachable = Vec::new();
        let mut frontier = vec![function_id.to_string()];
        for _ in 0..depth.min(budget.max_call_depth) {
            let mut next = Vec::new();
            for source in frontier {
                for target in self.get_function_callees(&source, budget)? {
                    if seen.contains(&target) {
                        continue;
                    }
                    seen.push(target.clone());
                    reachable.push(target.clone());
                    next.push(target);
                    if reachable.len() >= budget.max_callees {
                        return Ok(reachable);
                    }
                }
            }
            frontier = next;
            if frontier.is_empty() {
                break;
            }
        }
        Ok(reachable)
    }

    pub fn get_function_field_reads(&self, function_id: &str) -> rusqlite::Result<Vec<String>> {
        self.edge_targets_by_type(function_id, "ReadsField", i64::MAX)
    }

    pub fn get_function_field_writes(&self, function_id: &str) -> rusqlite::Result<Vec<String>> {
        self.edge_targets_by_type(function_id, "WritesField", i64::MAX)
    }

    pub fn get_operations_by_tag(
        &self,
        package_id: &str,
        tag: &str,
        budget: &ContextBudget,
    ) -> rusqlite::Result<Vec<Operation>> {
        let mut stmt = self.connection.prepare(
            "SELECT operations.id, operations.package_id, operations.function_id, operations.index_in_function, operations.kind, operations.display, operations.target, operations.source_span_json, operations.metadata_json
             FROM operations
             INNER JOIN semantic_tags ON semantic_tags.target_id = operations.id
             WHERE operations.package_id = ?1 AND semantic_tags.tag = ?2
             ORDER BY operations.function_id, operations.index_in_function
             LIMIT ?3",
        )?;
        let rows = stmt
            .query_map(
                params![package_id, tag, budget.max_operations as i64],
                operation_from_row,
            )?
            .collect();
        rows
    }

    pub fn get_functions_by_tag(
        &self,
        package_id: &str,
        tag: &str,
        budget: &ContextBudget,
    ) -> rusqlite::Result<Vec<SymbolResult>> {
        let mut stmt = self.connection.prepare(
            "SELECT functions.id, 'function', functions.full_name, functions.visibility, functions.is_entry
             FROM functions
             INNER JOIN semantic_tags ON semantic_tags.target_id = functions.id
             WHERE functions.package_id = ?1 AND semantic_tags.tag = ?2
             ORDER BY functions.full_name
             LIMIT ?3",
        )?;
        let rows = stmt
            .query_map(
                params![package_id, tag, budget.max_callees as i64],
                symbol_from_row,
            )?
            .collect();
        rows
    }

    pub fn get_public_entry_functions(
        &self,
        package_id: &str,
    ) -> rusqlite::Result<Vec<SymbolResult>> {
        let mut stmt = self.connection.prepare(
            "SELECT id, 'function', full_name, visibility, is_entry
             FROM functions
             WHERE package_id = ?1 AND is_entry = 1
             ORDER BY full_name",
        )?;
        let rows = stmt.query_map([package_id], symbol_from_row)?.collect();
        rows
    }

    pub fn search_symbols(
        &self,
        package_id: &str,
        query: &str,
        budget: &ContextBudget,
    ) -> rusqlite::Result<Vec<SymbolResult>> {
        let like = format!("%{query}%");
        let limit = budget.max_related_types.max(budget.max_callees).max(12) as i64;
        let mut stmt = self.connection.prepare(
            "SELECT id, kind, full_name, visibility, is_entry FROM (
               SELECT id, 'function' AS kind, full_name, visibility, is_entry FROM functions WHERE package_id = ?1 AND full_name LIKE ?2
               UNION ALL
               SELECT id, 'type' AS kind, full_name, kind AS visibility, 0 AS is_entry FROM types WHERE package_id = ?1 AND full_name LIKE ?2
               UNION ALL
               SELECT id, 'module' AS kind, full_name, '' AS visibility, 0 AS is_entry FROM modules WHERE package_id = ?1 AND full_name LIKE ?2
             ) ORDER BY full_name LIMIT ?3",
        )?;
        let rows = stmt
            .query_map(params![package_id, like, limit], symbol_from_row)?
            .collect();
        rows
    }

    pub fn get_summary_artifact_pointer(
        &self,
        package_alias: &str,
        module_name: &str,
    ) -> rusqlite::Result<Option<ModuleSummaryCard>> {
        self.connection
            .query_row(
                "SELECT id, package_alias, module_name, summary_path, content_hash, role, materialized_status, card_json
                 FROM summary_artifacts WHERE package_alias = ?1 AND module_name = ?2
                 ORDER BY last_seen_at DESC LIMIT 1",
                params![package_alias, module_name],
                module_card_from_row,
            )
            .optional()
    }

    pub fn get_context_pack(
        &self,
        target_id: &str,
        budget: &ContextBudget,
    ) -> rusqlite::Result<ContextPack> {
        if target_id.starts_with("function:") {
            let function = self.get_function_context(target_id, budget)?;
            let text = serde_json::to_string(&function).unwrap_or_default();
            return Ok(pack_from_sections(
                target_id,
                budget,
                vec![text],
                function.trimmed,
                function.trim_reasons,
            ));
        }
        if target_id.starts_with("module:") {
            let module = self.get_module_context(target_id)?;
            let text = serde_json::to_string(&module).unwrap_or_default();
            return Ok(pack_from_sections(
                target_id,
                budget,
                vec![text],
                false,
                Vec::new(),
            ));
        }
        if target_id.starts_with("type:") {
            let type_context = self.get_type_context(target_id)?;
            let text = serde_json::to_string(&type_context).unwrap_or_default();
            return Ok(pack_from_sections(
                target_id,
                budget,
                vec![text],
                false,
                Vec::new(),
            ));
        }

        Ok(pack_from_sections(
            target_id,
            budget,
            vec![format!(
                "{{\"targetId\":\"{target_id}\",\"status\":\"not_found\"}}"
            )],
            true,
            vec!["target_not_found".to_string()],
        ))
    }

    pub fn get_call_graph(
        &self,
        function_id: &str,
        depth: usize,
        budget: &ContextBudget,
    ) -> rusqlite::Result<GraphView> {
        let mut nodes = vec![function_id.to_string()];
        let mut edges = Vec::new();
        let mut frontier = vec![function_id.to_string()];
        for _ in 0..depth.min(budget.max_call_depth) {
            let mut next = Vec::new();
            for source in frontier {
                for target in self.get_function_callees(&source, budget)? {
                    edges.push((source.clone(), target.clone()));
                    if !nodes.contains(&target) && nodes.len() < budget.max_callees + 1 {
                        nodes.push(target.clone());
                        next.push(target);
                    }
                }
            }
            frontier = next;
        }
        let mut trimmed = false;
        let mut trim_reasons = Vec::new();
        if depth > budget.max_call_depth {
            trimmed = true;
            trim_reasons.push("call_depth_limited".to_string());
        }
        if nodes.len() >= budget.max_callees + 1 {
            trimmed = true;
            trim_reasons.push("callee_count_limited".to_string());
        }
        Ok(GraphView {
            nodes,
            edges,
            trimmed,
            trim_reasons,
        })
    }

    pub fn diagnostics_for_package(&self, package_id: &str) -> rusqlite::Result<Vec<Diagnostic>> {
        let mut stmt = self.connection.prepare(
            "SELECT id, package_id, severity, source, message, source_span_json, metadata_json
             FROM diagnostics WHERE package_id = ?1 ORDER BY severity, source, message",
        )?;
        let rows = stmt.query_map([package_id], diagnostic_from_row)?.collect();
        rows
    }

    fn load_module(&self, module_id: &str) -> rusqlite::Result<ModuleInfo> {
        self.connection.query_row(
            "SELECT id, package_id, summary_artifact_id, file_id, address, name, full_name, immediate_dependencies_json, docs, attributes_json, source_span_json
             FROM modules WHERE id = ?1",
            [module_id],
            module_from_row,
        )
    }

    fn load_type(&self, type_id: &str) -> rusqlite::Result<TypeDef> {
        let mut type_def = self.connection.query_row(
            "SELECT id, package_id, module_id, name, full_name, kind, abilities_json, type_parameters_json, docs, attributes_json, source_span_json
             FROM types WHERE id = ?1",
            [type_id],
            type_from_row,
        )?;
        type_def.fields = self.fields_for_type(type_id)?;
        Ok(type_def)
    }

    fn load_function(&self, function_id: &str) -> rusqlite::Result<FunctionInfo> {
        self.connection.query_row(
            "SELECT id, package_id, module_id, name, full_name, visibility, is_entry, is_native, type_parameters_json, parameters_json, returns_json, acquires_json, docs, attributes_json, source_span_json
             FROM functions WHERE id = ?1",
            [function_id],
            function_from_row,
        )
    }

    fn symbols_for_module(
        &self,
        module_id: &str,
        kind: &str,
    ) -> rusqlite::Result<Vec<SymbolResult>> {
        let sql = if kind == "function" {
            "SELECT id, 'function', full_name, visibility, is_entry FROM functions WHERE module_id = ?1 ORDER BY full_name"
        } else {
            "SELECT id, 'type', full_name, kind, 0 FROM types WHERE module_id = ?1 ORDER BY full_name"
        };
        let mut stmt = self.connection.prepare(sql)?;
        let rows = stmt.query_map([module_id], symbol_from_row)?.collect();
        rows
    }

    fn fields_for_type(&self, type_id: &str) -> rusqlite::Result<Vec<crate::core::FieldInfo>> {
        let mut stmt = self.connection.prepare(
            "SELECT id, package_id, module_id, type_id, name, type_name, source_span_json
             FROM fields WHERE type_id = ?1 ORDER BY name",
        )?;
        let rows = stmt.query_map([type_id], field_from_row)?.collect();
        rows
    }

    fn tags_for_target(&self, target_id: &str) -> rusqlite::Result<Vec<SemanticTag>> {
        let mut stmt = self.connection.prepare(
            "SELECT id, package_id, target_id, tag, source_span_json, metadata_json
             FROM semantic_tags WHERE target_id = ?1 ORDER BY tag",
        )?;
        let rows = stmt
            .query_map([target_id], semantic_tag_from_row)?
            .collect();
        rows
    }

    fn operations_for_function(
        &self,
        function_id: &str,
        limit: usize,
    ) -> rusqlite::Result<Vec<Operation>> {
        let mut stmt = self.connection.prepare(
            "SELECT id, package_id, function_id, index_in_function, kind, display, target, source_span_json, metadata_json
             FROM operations WHERE function_id = ?1 ORDER BY index_in_function LIMIT ?2",
        )?;
        let rows = stmt
            .query_map(params![function_id, limit as i64], operation_from_row)?
            .collect();
        rows
    }

    fn operation_count(&self, function_id: &str) -> rusqlite::Result<usize> {
        self.connection
            .query_row(
                "SELECT COUNT(*) FROM operations WHERE function_id = ?1",
                [function_id],
                |row| row.get::<_, i64>(0),
            )
            .map(|count| count as usize)
    }

    fn function_evidence_summary(
        &self,
        function: &FunctionInfo,
        operation_count: usize,
        field_read_count: usize,
        field_write_count: usize,
    ) -> rusqlite::Result<FunctionEvidenceSummary> {
        let exact_operation_spans =
            self.operation_source_precision_count(&function.id, "ExactExpression")?;
        let source_mapped_operations = self.connection.query_row(
            "SELECT COUNT(*) FROM operations WHERE function_id = ?1 AND source_span_json NOT LIKE '%\"precision\":\"Unknown\"%'",
            [&function.id],
            |row| row.get::<_, i64>(0),
        )? as usize;
        let call_operation_count = self.connection.query_row(
            "SELECT COUNT(*) FROM operations WHERE function_id = ?1 AND kind = 'Call'",
            [&function.id],
            |row| row.get::<_, i64>(0),
        )? as usize;
        let call_edge_count = self.connection.query_row(
            "SELECT COUNT(*) FROM edges WHERE from_id = ?1 AND edge_type = 'Calls' AND operation_id IS NOT NULL",
            [&function.id],
            |row| row.get::<_, i64>(0),
        )? as usize;

        Ok(FunctionEvidenceSummary {
            body_indexed: operation_count > 0,
            operation_count,
            exact_operation_spans,
            source_mapped_operations,
            call_operation_count,
            call_edge_count,
            field_read_count,
            field_write_count,
            source_precision: format!("{:?}", function.source_span.precision),
        })
    }

    fn operation_source_precision_count(
        &self,
        function_id: &str,
        precision: &str,
    ) -> rusqlite::Result<usize> {
        let pattern = format!("%\"precision\":\"{precision}\"%");
        self.connection
            .query_row(
                "SELECT COUNT(*) FROM operations WHERE function_id = ?1 AND source_span_json LIKE ?2",
                params![function_id, pattern],
                |row| row.get::<_, i64>(0),
            )
            .map(|count| count as usize)
    }

    fn direct_calls(&self, function_id: &str, limit: usize) -> rusqlite::Result<Vec<String>> {
        self.edge_neighbors("from_id", "to_id", function_id, limit)
    }

    fn edge_targets_by_type(
        &self,
        function_id: &str,
        edge_type: &str,
        limit: i64,
    ) -> rusqlite::Result<Vec<String>> {
        let mut stmt = self.connection.prepare(
            "SELECT DISTINCT to_id FROM edges WHERE from_id = ?1 AND edge_type = ?2 ORDER BY to_id LIMIT ?3",
        )?;
        let rows = stmt
            .query_map(params![function_id, edge_type, limit], |row| row.get(0))?
            .collect();
        rows
    }

    fn operation_histogram(
        &self,
        function_id: &str,
    ) -> rusqlite::Result<Vec<OperationHistogramEntry>> {
        let mut stmt = self.connection.prepare(
            "SELECT kind, COUNT(*) FROM operations WHERE function_id = ?1 GROUP BY kind ORDER BY kind",
        )?;
        let rows = stmt
            .query_map([function_id], |row| {
                Ok(OperationHistogramEntry {
                    kind: row.get(0)?,
                    count: row.get::<_, i64>(1)? as usize,
                })
            })?
            .collect();
        rows
    }

    fn related_types_for_function(
        &self,
        function: &FunctionInfo,
        limit: usize,
    ) -> rusqlite::Result<Vec<RelatedTypeCard>> {
        let type_text = function
            .parameters
            .iter()
            .map(|parameter| parameter.type_name.as_str())
            .chain(function.returns.iter().map(String::as_str))
            .chain(function.acquires.iter().map(String::as_str))
            .collect::<Vec<_>>()
            .join("\n");
        if type_text.is_empty() || limit == 0 {
            return Ok(Vec::new());
        }

        let mut stmt = self.connection.prepare(
            "SELECT id, package_id, module_id, name, full_name, kind, abilities_json, type_parameters_json, docs, attributes_json, source_span_json
             FROM types WHERE package_id = ?1 ORDER BY full_name",
        )?;
        let mut related = Vec::new();
        let rows = stmt.query_map([function.package_id.as_str()], type_from_row)?;
        for row in rows {
            let mut type_def = row?;
            if !type_text.contains(&type_def.name) && !type_text.contains(&type_def.full_name) {
                continue;
            }
            type_def.fields = self.fields_for_type(&type_def.id)?;
            related.push(RelatedTypeCard {
                id: type_def.id,
                full_name: type_def.full_name,
                kind: format!("{:?}", type_def.kind),
                abilities: type_def.abilities,
                fields: type_def
                    .fields
                    .into_iter()
                    .map(|field| format!("{}: {}", field.name, field.type_name))
                    .collect(),
            });
            if related.len() >= limit {
                break;
            }
        }
        Ok(related)
    }

    fn source_excerpts_for_function(
        &self,
        function: &FunctionInfo,
        budget: &ContextBudget,
    ) -> rusqlite::Result<Vec<SourceExcerpt>> {
        let Some(file_id) = function.source_span.file_id.as_deref() else {
            return Ok(Vec::new());
        };
        let Some(path) = self.file_path(file_id)? else {
            return Ok(Vec::new());
        };
        let Ok(source) = fs::read_to_string(path) else {
            return Ok(Vec::new());
        };
        let lines = source.lines().collect::<Vec<_>>();
        let total_lines = lines.len() as u32;
        if total_lines == 0 {
            return Ok(Vec::new());
        }

        let start = function.source_span.start_line.unwrap_or(1).max(1);
        let mut end = function.source_span.end_line.unwrap_or(start).max(start);
        if !budget.include_full_source {
            let max_end =
                start.saturating_add(budget.max_source_excerpt_lines.saturating_sub(1) as u32);
            end = end.min(max_end);
        }
        end = end.min(total_lines);
        let text = lines[(start - 1) as usize..end as usize].join("\n");
        Ok(vec![SourceExcerpt {
            file_id: file_id.to_string(),
            start_line: start,
            end_line: end,
            precision: format!("{:?}", function.source_span.precision),
            text,
        }])
    }

    fn file_path(&self, file_id: &str) -> rusqlite::Result<Option<String>> {
        self.connection
            .query_row("SELECT path FROM files WHERE id = ?1", [file_id], |row| {
                row.get(0)
            })
            .optional()
    }

    fn edge_neighbors(
        &self,
        source_column: &str,
        target_column: &str,
        id: &str,
        limit: usize,
    ) -> rusqlite::Result<Vec<String>> {
        let sql = format!(
            "SELECT {target_column} FROM edges WHERE {source_column} = ?1 AND edge_type = 'Calls' ORDER BY {target_column} LIMIT ?2"
        );
        let mut stmt = self.connection.prepare(&sql)?;
        let rows = stmt
            .query_map(params![id, limit as i64], |row| row.get(0))?
            .collect();
        rows
    }
}

fn count(connection: &Connection, table: &str, package_id: &str) -> rusqlite::Result<usize> {
    let sql = format!("SELECT COUNT(*) FROM {table} WHERE package_id = ?1");
    connection
        .query_row(&sql, [package_id], |row| row.get::<_, i64>(0))
        .map(|count| count as usize)
}

fn apply_function_budget(context: &mut FunctionContext) {
    refresh_estimate(context);
    if context.estimated_tokens > context.budget_tokens {
        let mut stripped = 0;
        for operation in &mut context.operations {
            if operation.metadata_json.take().is_some() {
                stripped += 1;
            }
        }
        if stripped > 0 {
            context.trimmed = true;
            context
                .trim_reasons
                .push("operation_metadata_trimmed".to_string());
            refresh_estimate(context);
        }
    }
    if context.estimated_tokens > context.budget_tokens && !context.source_excerpts.is_empty() {
        context.trimmed = true;
        context
            .trim_reasons
            .push("source_excerpts_trimmed".to_string());
        for excerpt in &mut context.source_excerpts {
            let lines = excerpt
                .text
                .lines()
                .take(8)
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>();
            excerpt.text = lines.join("\n");
            excerpt.end_line = excerpt
                .start_line
                .saturating_add(lines.len().saturating_sub(1) as u32);
        }
        refresh_estimate(context);
    }
    while context.estimated_tokens > context.budget_tokens && !context.operations.is_empty() {
        if let Some(index) = context
            .operations
            .iter()
            .rposition(|operation| !is_high_signal_operation(&operation.kind))
        {
            context.trimmed = true;
            context
                .trim_reasons
                .push("low_signal_operations_collapsed".to_string());
            context.operations.remove(index);
        } else {
            break;
        }
        refresh_estimate(context);
    }
    if context.estimated_tokens > context.budget_tokens && !context.callers.is_empty() {
        context.trimmed = true;
        context.trim_reasons.push("callers_trimmed".to_string());
        context.callers.clear();
        refresh_estimate(context);
    }
    if context.estimated_tokens > context.budget_tokens && !context.callees.is_empty() {
        context.trimmed = true;
        context.trim_reasons.push("callees_trimmed".to_string());
        context.callees.clear();
        context.outline.direct_calls.clear();
        refresh_estimate(context);
    }
    if context.estimated_tokens > context.budget_tokens && !context.reachable_callees.is_empty() {
        context.trimmed = true;
        context
            .trim_reasons
            .push("reachable_callees_trimmed".to_string());
        context.reachable_callees.clear();
        refresh_estimate(context);
    }
    if context.estimated_tokens > context.budget_tokens && !context.related_types.is_empty() {
        context.trimmed = true;
        context
            .trim_reasons
            .push("related_types_trimmed".to_string());
        context.related_types.clear();
        refresh_estimate(context);
    }
    if context.estimated_tokens > context.budget_tokens && !context.diagnostics.is_empty() {
        context.trimmed = true;
        context.trim_reasons.push("diagnostics_trimmed".to_string());
        context.diagnostics.clear();
        refresh_estimate(context);
    }
    if context.estimated_tokens > context.budget_tokens && !context.source_excerpts.is_empty() {
        context.trimmed = true;
        context
            .trim_reasons
            .push("source_excerpts_removed".to_string());
        context.source_excerpts.clear();
        refresh_estimate(context);
    }
    if context.estimated_tokens > context.budget_tokens && !context.operation_histogram.is_empty() {
        context.trimmed = true;
        context
            .trim_reasons
            .push("operation_histogram_trimmed".to_string());
        context.operation_histogram.clear();
        refresh_estimate(context);
    }
    if context.estimated_tokens > context.budget_tokens
        && (!context.field_reads.is_empty() || !context.field_writes.is_empty())
    {
        context.trimmed = true;
        context
            .trim_reasons
            .push("field_access_lists_trimmed".to_string());
        context.field_reads.clear();
        context.field_writes.clear();
        refresh_estimate(context);
    }
    if context.estimated_tokens > context.budget_tokens {
        let mut stripped = 0;
        for operation in &mut context.operations {
            if operation.source_span.precision != crate::core::SourcePrecision::Unknown {
                operation.source_span = SourceSpan::unknown();
                stripped += 1;
            }
        }
        if stripped > 0 {
            context.trimmed = true;
            context
                .trim_reasons
                .push("operation_spans_trimmed".to_string());
            refresh_estimate(context);
        }
    }
    while context.estimated_tokens > context.budget_tokens && !context.operations.is_empty() {
        context.trimmed = true;
        if let Some(index) = context
            .operations
            .iter()
            .rposition(|operation| is_support_operation(&operation.kind))
        {
            context
                .trim_reasons
                .push("support_operations_trimmed".to_string());
            context.operations.remove(index);
        } else {
            context.trim_reasons.push("operations_trimmed".to_string());
            context.operations.pop();
        }
        refresh_estimate(context);
    }
}

fn refresh_estimate(context: &mut FunctionContext) {
    context.trim_reasons.sort();
    context.trim_reasons.dedup();
    let text = serde_json::to_string(context).unwrap_or_default();
    context.estimated_tokens = estimate_tokens(&text);
}

fn is_high_signal_operation(kind: &OperationKind) -> bool {
    matches!(
        kind,
        OperationKind::Call
            | OperationKind::Assert
            | OperationKind::Abort
            | OperationKind::ReadField
            | OperationKind::WriteField
            | OperationKind::BorrowFieldMut
            | OperationKind::BorrowGlobalMut
            | OperationKind::MoveFrom
            | OperationKind::MoveTo
            | OperationKind::Pack
            | OperationKind::Unpack
            | OperationKind::Branch
            | OperationKind::BranchIf
    )
}

fn is_support_operation(kind: &OperationKind) -> bool {
    !matches!(
        kind,
        OperationKind::Call
            | OperationKind::Assert
            | OperationKind::ReadField
            | OperationKind::WriteField
            | OperationKind::BorrowFieldMut
            | OperationKind::BorrowGlobalMut
            | OperationKind::MoveFrom
            | OperationKind::MoveTo
            | OperationKind::Pack
            | OperationKind::Unpack
    )
}

fn pack_from_sections(
    target_id: &str,
    budget: &ContextBudget,
    mut sections: Vec<String>,
    mut trimmed: bool,
    mut trim_reasons: Vec<String>,
) -> ContextPack {
    let mut text = sections.join("\n");
    let mut estimated_tokens = estimate_tokens(&text);
    if estimated_tokens > budget.max_tokens_estimate {
        trimmed = true;
        trim_reasons.push("context_pack_truncated".to_string());
        let max_chars = budget.max_tokens_estimate.saturating_mul(4);
        text.truncate(max_chars);
        sections = vec![text.clone()];
        estimated_tokens = estimate_tokens(&text);
    }
    ContextPack {
        target_id: target_id.to_string(),
        level: budget.level,
        sections,
        estimated_tokens,
        budget_tokens: budget.max_tokens_estimate,
        trimmed,
        trim_reasons,
    }
}

fn module_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ModuleInfo> {
    Ok(ModuleInfo {
        id: row.get(0)?,
        package_id: row.get(1)?,
        summary_artifact_id: row.get(2)?,
        file_id: row.get(3)?,
        address: row.get(4)?,
        name: row.get(5)?,
        full_name: row.get(6)?,
        immediate_dependencies: json_from_col(row, 7)?,
        docs: row.get(8)?,
        attributes: json_from_col(row, 9)?,
        source_span: json_from_col(row, 10)?,
    })
}

fn type_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<TypeDef> {
    Ok(TypeDef {
        id: row.get(0)?,
        package_id: row.get(1)?,
        module_id: row.get(2)?,
        name: row.get(3)?,
        full_name: row.get(4)?,
        kind: parse_type_kind(&row.get::<_, String>(5)?),
        abilities: json_from_col(row, 6)?,
        type_parameters: json_from_col(row, 7)?,
        fields: Vec::new(),
        docs: row.get(8)?,
        attributes: json_from_col(row, 9)?,
        source_span: json_from_col(row, 10)?,
    })
}

fn field_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<crate::core::FieldInfo> {
    Ok(crate::core::FieldInfo {
        id: row.get(0)?,
        package_id: row.get(1)?,
        module_id: row.get(2)?,
        type_id: row.get(3)?,
        name: row.get(4)?,
        type_name: row.get(5)?,
        source_span: json_from_col(row, 6)?,
    })
}

fn function_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<FunctionInfo> {
    Ok(FunctionInfo {
        id: row.get(0)?,
        package_id: row.get(1)?,
        module_id: row.get(2)?,
        name: row.get(3)?,
        full_name: row.get(4)?,
        visibility: parse_visibility(&row.get::<_, String>(5)?),
        is_entry: row.get::<_, i64>(6)? != 0,
        is_native: row.get::<_, i64>(7)? != 0,
        type_parameters: json_from_col(row, 8)?,
        parameters: json_from_col::<Vec<FunctionParameter>>(row, 9)?,
        returns: json_from_col(row, 10)?,
        acquires: json_from_col(row, 11)?,
        docs: row.get(12)?,
        attributes: json_from_col(row, 13)?,
        source_span: json_from_col(row, 14)?,
    })
}

fn operation_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Operation> {
    Ok(Operation {
        id: row.get(0)?,
        package_id: row.get(1)?,
        function_id: row.get(2)?,
        index_in_function: row.get::<_, i64>(3)? as usize,
        kind: parse_operation_kind(&row.get::<_, String>(4)?),
        display: row.get(5)?,
        target: row.get(6)?,
        source_span: json_from_col(row, 7)?,
        metadata_json: optional_json_from_col(row, 8)?,
    })
}

fn semantic_tag_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<SemanticTag> {
    Ok(SemanticTag {
        id: row.get(0)?,
        package_id: row.get(1)?,
        target_id: row.get(2)?,
        tag: row.get(3)?,
        source_span: json_from_col(row, 4)?,
        metadata_json: optional_json_from_col(row, 5)?,
    })
}

fn diagnostic_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Diagnostic> {
    Ok(Diagnostic {
        id: row.get(0)?,
        package_id: row.get(1)?,
        severity: match row.get::<_, String>(2)?.as_str() {
            "Error" => crate::core::DiagnosticSeverity::Error,
            "Warning" => crate::core::DiagnosticSeverity::Warning,
            _ => crate::core::DiagnosticSeverity::Info,
        },
        source: row.get(3)?,
        message: row.get(4)?,
        source_span: json_from_col(row, 5)?,
        metadata_json: optional_json_from_col(row, 6)?,
    })
}

fn symbol_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<SymbolResult> {
    Ok(SymbolResult {
        id: row.get(0)?,
        kind: row.get(1)?,
        full_name: row.get(2)?,
        visibility: row.get(3)?,
        is_entry: row.get::<_, i64>(4)? != 0,
    })
}

fn module_card_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ModuleSummaryCard> {
    Ok(ModuleSummaryCard {
        artifact_id: row.get(0)?,
        package_alias: row.get(1)?,
        module_name: row.get(2)?,
        summary_path: row.get(3)?,
        content_hash: row.get(4)?,
        role: row.get(5)?,
        materialized_status: row.get(6)?,
        card: optional_json_from_col(row, 7)?,
        estimated_tokens: 0,
        budget_tokens: 0,
        trimmed: false,
        trim_reasons: Vec::new(),
    })
}

fn json_from_col<T: serde::de::DeserializeOwned>(
    row: &rusqlite::Row<'_>,
    index: usize,
) -> rusqlite::Result<T> {
    let value: String = row.get(index)?;
    serde_json::from_str(&value).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(index, rusqlite::types::Type::Text, error.into())
    })
}

fn optional_json_from_col(
    row: &rusqlite::Row<'_>,
    index: usize,
) -> rusqlite::Result<Option<serde_json::Value>> {
    let value: Option<String> = row.get(index)?;
    value
        .map(|value| {
            serde_json::from_str(&value).map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    index,
                    rusqlite::types::Type::Text,
                    error.into(),
                )
            })
        })
        .transpose()
}

fn parse_visibility(value: &str) -> FunctionVisibility {
    match value {
        "Public" => FunctionVisibility::Public,
        "PublicFriend" => FunctionVisibility::PublicFriend,
        "PublicPackage" => FunctionVisibility::PublicPackage,
        "Private" => FunctionVisibility::Private,
        "Native" => FunctionVisibility::Native,
        _ => FunctionVisibility::Unknown,
    }
}

fn parse_type_kind(value: &str) -> TypeKind {
    match value {
        "Struct" => TypeKind::Struct,
        "Enum" => TypeKind::Enum,
        "Native" => TypeKind::Native,
        _ => TypeKind::Unknown,
    }
}

fn parse_operation_kind(value: &str) -> OperationKind {
    match value {
        "Nop" => OperationKind::Nop,
        "Assign" => OperationKind::Assign,
        "Copy" => OperationKind::Copy,
        "Move" => OperationKind::Move,
        "Call" => OperationKind::Call,
        "Return" => OperationKind::Return,
        "Abort" => OperationKind::Abort,
        "Assert" => OperationKind::Assert,
        "Branch" => OperationKind::Branch,
        "BranchIf" => OperationKind::BranchIf,
        "CompareEq" => OperationKind::CompareEq,
        "CompareNeq" => OperationKind::CompareNeq,
        "CompareLt" => OperationKind::CompareLt,
        "CompareGt" => OperationKind::CompareGt,
        "CompareLe" => OperationKind::CompareLe,
        "CompareGe" => OperationKind::CompareGe,
        "BorrowLocal" => OperationKind::BorrowLocal,
        "BorrowField" => OperationKind::BorrowField,
        "BorrowFieldMut" => OperationKind::BorrowFieldMut,
        "ReadField" => OperationKind::ReadField,
        "WriteField" => OperationKind::WriteField,
        "Pack" => OperationKind::Pack,
        "Unpack" => OperationKind::Unpack,
        "CreateStruct" => OperationKind::CreateStruct,
        "DestroyStruct" => OperationKind::DestroyStruct,
        "FreezeRef" => OperationKind::FreezeRef,
        "BorrowGlobal" => OperationKind::BorrowGlobal,
        "BorrowGlobalMut" => OperationKind::BorrowGlobalMut,
        "MoveFrom" => OperationKind::MoveFrom,
        "MoveTo" => OperationKind::MoveTo,
        "VectorOp" => OperationKind::VectorOp,
        "Constant" => OperationKind::Constant,
        "Add" => OperationKind::Add,
        "Sub" => OperationKind::Sub,
        "Mul" => OperationKind::Mul,
        "Div" => OperationKind::Div,
        "Mod" => OperationKind::Mod,
        _ => OperationKind::Unknown,
    }
}
