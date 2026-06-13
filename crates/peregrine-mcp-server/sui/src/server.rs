use crate::command;
use crate::{
    adapter::{SecuritySuiCommandKind, build_sui_move_new_command, build_sui_package_command},
    analysis::{
        apply_analyze_args, ensure_required_stages, legacy_static_report, scanner_report_value,
        static_rule_catalog, sui_bytecode_decompile, sui_bytecode_view, sui_modules,
        sui_package_insights, sui_signatures, sui_test_scanner_report,
    },
    artifacts::{MovePackageContext, resolve_move_package},
    dynamic::dynamic_command_result,
    graphs::{legacy_project_graphs, legacy_state_graph},
};
use peregrine_analysis::{
    AnalysisOptions, AnalysisReport as EngineAnalysisReport, AnalysisRequest, AnalysisStage,
    AnalysisTarget, GraphKind,
};
use peregrine_analysis_engine::AnalysisEngine;
use peregrine_sui_adapter::{
    SuiAdapterSettings as AdapterSettings, SuiAdapterSource as AdapterSource,
};
use peregrine_sui_import_engine::{
    BuildVerification, BuildableImportRequest, ImportEngine, ImportEngineConfig,
    default_import_root,
};
use peregrine_sui_mcp_protocol::{
    AnalysisRuleCatalog as ProtocolAnalysisRuleCatalog, AnalyzeArgs, AnalyzeTargetArgs,
    BytecodeViewResponse, CreatePackageArgs, DEFAULT_COMMAND_TIMEOUT_MS,
    DEFAULT_FORMAL_VERIFY_TIMEOUT_SECONDS, DEFAULT_MOVY_FUZZ_SEED,
    DEFAULT_MOVY_FUZZ_TIME_LIMIT_SECONDS, DEFAULT_PAGE_SIZE, EngineAnalysisResponse,
    FormalVerifyArgs, FunctionStateGraphArgs, FunctionStateGraphResponse, GraphsResponse,
    ImportArtifact, ImportPackageArgs, ImportPackageResponse, MAX_PAGE_SIZE, ModulesArgs,
    ModulesPage, MoveBytecodePackageView, MovyFuzzArgs, PackageArgs, PackageSummary,
    SignaturesArgs, SignaturesPage, StaticAnalysisArgs, StaticAnalysisResponse,
    StaticRuleCatalogResponse, SuiAdapterSettings, SuiAdapterSource, SuiCommandArgs,
    TestScannerArgs, TestScannerResponse, tool_definitions, tool_name,
};
use peregrine_sui_project_loader::{sui_analysis_engine, sui_analysis_engine_with_settings};
use rmcp::{
    ErrorData, ServerHandler,
    model::{
        CallToolRequestParams, CallToolResult, Content, Implementation, JsonObject,
        ListToolsResult, PaginatedRequestParams, ServerCapabilities, ServerInfo, Tool,
        ToolAnnotations,
    },
    service::{RequestContext, RoleServer},
};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::{
    borrow::Cow,
    path::{Component, Path, PathBuf},
    sync::Arc,
};

#[derive(Clone)]
pub struct PeregrineMcpServer {
    workspace_root: PathBuf,
    adapter_settings: SuiAdapterSettings,
    analysis_engine: AnalysisEngine,
}

impl PeregrineMcpServer {
    pub fn new(workspace_root: PathBuf) -> anyhow::Result<Self> {
        let workspace_root = workspace_root.canonicalize()?;
        Ok(Self {
            workspace_root,
            adapter_settings: SuiAdapterSettings::default(),
            analysis_engine: sui_analysis_engine()?,
        })
    }

    pub fn with_adapter_settings(
        mut self,
        adapter_settings: SuiAdapterSettings,
    ) -> anyhow::Result<Self> {
        self.analysis_engine =
            sui_analysis_engine_with_settings(to_adapter_settings(&adapter_settings))?;
        self.adapter_settings = adapter_settings;
        Ok(self)
    }

    fn resolve_context(&self, args: &PackageArgs) -> Result<MovePackageContext, String> {
        let project_root = self.resolve_project_root(args.project_root.as_deref())?;
        resolve_move_package(project_root, args.package_path.as_deref())
            .map_err(|error| error.to_string())
    }

    async fn run_package_analysis(
        &self,
        context: &MovePackageContext,
        stages: Vec<AnalysisStage>,
        graph_kinds: Vec<GraphKind>,
        dynamic_capabilities: Vec<String>,
        mut options: AnalysisOptions,
    ) -> EngineAnalysisReport {
        options.insert("projectRoot".to_string(), json!(context.project_root));
        options.insert("packagePath".to_string(), json!(context.package_path));
        let mut request = AnalysisRequest::safe(
            peregrine_analysis::ChainId::new("sui"),
            AnalysisTarget::LocalPackage {
                path: context.package_root.clone(),
            },
        );
        request.stages = stages;
        request.graph_kinds = graph_kinds;
        request.dynamic_capabilities = dynamic_capabilities;
        request.options = options;
        ensure_required_stages(&mut request.stages);
        self.analysis_engine.run(request).await
    }

    fn resolve_project_root(&self, project_root: Option<&str>) -> Result<PathBuf, String> {
        let project_root = project_root
            .filter(|value| !value.trim().is_empty())
            .map_or_else(
                || self.workspace_root.clone(),
                |value| self.workspace_root.join(value),
            )
            .canonicalize()
            .map_err(|error| format!("failed to resolve project_root: {error}"))?;
        if !project_root.starts_with(&self.workspace_root) {
            return Err(format!(
                "project_root must remain inside the MCP workspace {}",
                self.workspace_root.display()
            ));
        }
        if !project_root.is_dir() {
            return Err("project_root must be a directory".to_string());
        }
        Ok(project_root)
    }

    async fn dispatch(&self, request: CallToolRequestParams) -> Result<CallToolResult, ErrorData> {
        let arguments = request.arguments.unwrap_or_default();
        let value = match request.name.as_ref() {
            tool_name::PACKAGE_RESOLVE => {
                let args = parse_args::<PackageArgs>(&arguments)?;
                let ctx = self.resolve_context(&args).map_err(tool_error)?;
                json!({
                    "status": "ok",
                    "package": package_summary(&ctx),
                })
            }
            tool_name::MODULES => {
                let args = parse_args::<ModulesArgs>(&arguments)?;
                let ctx = self.resolve_context(&args.package).map_err(tool_error)?;
                let (source, modules) =
                    sui_modules(&ctx).map_err(|error| tool_error(error.to_string()))?;
                let modules = modules
                    .into_iter()
                    .filter(|module| {
                        args.modules.is_empty()
                            || args.modules.iter().any(|requested| {
                                module.module_name == *requested
                                    || module.module_address.as_deref().is_some_and(|address| {
                                        format!("{address}::{}", module.module_name) == *requested
                                    })
                            })
                    })
                    .filter(|module| {
                        args.file
                            .as_deref()
                            .is_none_or(|file| module.file_path == file)
                    })
                    .collect::<Vec<_>>();
                let offset = decode_cursor(args.cursor.as_deref())?;
                let limit = args
                    .limit
                    .unwrap_or(DEFAULT_PAGE_SIZE)
                    .clamp(1, MAX_PAGE_SIZE);
                let data = modules
                    .iter()
                    .skip(offset)
                    .take(limit)
                    .cloned()
                    .collect::<Vec<_>>();
                let next_offset = offset.saturating_add(data.len());
                let next_cursor = (next_offset < modules.len()).then(|| next_offset.to_string());
                serde_json::to_value(ModulesPage {
                    status: "ok".to_string(),
                    package: package_summary(&ctx),
                    source,
                    data,
                    next_cursor,
                })
                .map_err(serialization_error)?
            }
            tool_name::SIGNATURES => {
                let args = parse_args::<SignaturesArgs>(&arguments)?;
                let ctx = self.resolve_context(&args.package).map_err(tool_error)?;
                let module_filters = args
                    .modules
                    .iter()
                    .map(|module| module.trim())
                    .filter(|module| !module.is_empty())
                    .collect::<Vec<_>>();
                let file_filter = args
                    .file
                    .as_deref()
                    .map(str::trim)
                    .filter(|file| !file.is_empty());
                let signatures = sui_signatures(&ctx)
                    .map_err(|error| tool_error(error.to_string()))?
                    .into_iter()
                    .filter(|signature| {
                        module_filters.is_empty()
                            || module_filters.iter().any(|module| {
                                signature.module_name == *module
                                    || signature.module_address.as_deref().is_some_and(|address| {
                                        format!("{address}::{}", signature.module_name) == *module
                                    })
                            })
                    })
                    .filter(|signature| file_filter.is_none_or(|file| signature.file_path == file))
                    .collect::<Vec<_>>();
                let offset = decode_cursor(args.cursor.as_deref())?;
                let limit = args
                    .limit
                    .unwrap_or(DEFAULT_PAGE_SIZE)
                    .clamp(1, MAX_PAGE_SIZE);
                let data = signatures
                    .iter()
                    .skip(offset)
                    .take(limit)
                    .cloned()
                    .collect::<Vec<_>>();
                let next_offset = offset.saturating_add(data.len());
                let next_cursor = (next_offset < signatures.len()).then(|| next_offset.to_string());
                serde_json::to_value(SignaturesPage {
                    status: "ok".to_string(),
                    package: package_summary(&ctx),
                    data,
                    next_cursor,
                })
                .map_err(serialization_error)?
            }
            tool_name::IMPORT_PACKAGE => {
                let args = parse_args::<ImportPackageArgs>(&arguments)?;
                let project_root = self
                    .resolve_project_root(args.project_root.as_deref())
                    .map_err(tool_error)?;
                let import_root = match args.output_path.as_deref() {
                    Some(output_path) => {
                        workspace_output_path(&project_root, output_path).map_err(tool_error)?
                    }
                    None => default_import_root(&project_root, &args.network_id, &args.package_id)
                        .map_err(tool_error)?,
                };
                let request = BuildableImportRequest {
                    network_id: args.network_id,
                    graph_ql_url: args.graph_ql_url,
                    package_id: args.package_id,
                    import_root: import_root.clone(),
                    generate_buildable: !args.raw_only,
                };
                let engine = ImportEngine::new(ImportEngineConfig {
                    max_dependency_depth: args.max_dependency_depth.unwrap_or(3).min(16),
                    max_dependency_packages: args
                        .max_dependency_packages
                        .unwrap_or(64)
                        .clamp(1, 512),
                    build_verification: BuildVerification::Disabled,
                });
                let artifact = engine
                    .import_buildable_package(request)
                    .await
                    .map_err(tool_error)?;
                let artifact = serde_json::from_value::<ImportArtifact>(
                    serde_json::to_value(artifact).map_err(serialization_error)?,
                )
                .map_err(serialization_error)?;
                serde_json::to_value(ImportPackageResponse {
                    status: "ok".to_string(),
                    import_root: import_root.display().to_string(),
                    artifact,
                })
                .map_err(serialization_error)?
            }
            tool_name::CREATE_PACKAGE => {
                let args = parse_args::<CreatePackageArgs>(&arguments)?;
                let project_root = self
                    .resolve_project_root(args.project_root.as_deref())
                    .map_err(tool_error)?;
                let adapter_settings = to_adapter_settings(&self.adapter_settings);
                let security_command = build_sui_move_new_command(
                    &project_root,
                    &adapter_settings,
                    &args.package_name,
                )
                .map_err(|error| tool_error(error.to_string()))?;
                let package_root = project_root.join(args.package_name.trim());
                let summary = PackageSummary {
                    project_root: project_root.display().to_string(),
                    package_root: package_root.display().to_string(),
                    package_path: args.package_name.trim().to_string(),
                    package_name: args.package_name.trim().to_string(),
                };
                serde_json::to_value(
                    command::run(
                        security_command,
                        summary,
                        args.timeout_ms.unwrap_or(DEFAULT_COMMAND_TIMEOUT_MS),
                    )
                    .await,
                )
                .map_err(serialization_error)?
            }
            tool_name::STATIC_RULE_CATALOG => {
                let args = parse_args::<StaticAnalysisArgs>(&arguments)?;
                let ctx = self.resolve_context(&args.package).map_err(tool_error)?;
                let catalog = static_rule_catalog(&ctx, &args)
                    .map_err(|error| tool_error(error.to_string()))?;
                let catalog = serde_json::from_value::<ProtocolAnalysisRuleCatalog>(
                    serde_json::to_value(catalog).map_err(serialization_error)?,
                )
                .map_err(serialization_error)?;
                serde_json::to_value(StaticRuleCatalogResponse {
                    status: "ok".to_string(),
                    package: package_summary(&ctx),
                    catalog,
                })
                .map_err(serialization_error)?
            }
            tool_name::STATIC_ANALYZE_PACKAGE => {
                let args = parse_args::<StaticAnalysisArgs>(&arguments)?;
                let ctx = self.resolve_context(&args.package).map_err(tool_error)?;
                let report = self
                    .run_package_analysis(
                        &ctx,
                        vec![
                            AnalysisStage::Scan,
                            AnalysisStage::Graph,
                            AnalysisStage::Static,
                        ],
                        GraphKind::required(),
                        Vec::new(),
                        AnalysisOptions::from([
                            ("noGlobalPlugins".to_string(), json!(args.no_global_plugins)),
                            ("pluginPaths".to_string(), json!(args.plugins)),
                            ("rulesets".to_string(), json!(args.rulesets)),
                        ]),
                    )
                    .await;
                let report = legacy_static_report(&report)?;
                serde_json::to_value(StaticAnalysisResponse {
                    status: "ok".to_string(),
                    package: package_summary(&ctx),
                    report,
                })
                .map_err(serialization_error)?
            }
            tool_name::SCANNER_REPORT => {
                let args = parse_args::<PackageArgs>(&arguments)?;
                let ctx = self.resolve_context(&args).map_err(tool_error)?;
                let report = self
                    .run_package_analysis(
                        &ctx,
                        vec![AnalysisStage::Scan],
                        Vec::new(),
                        Vec::new(),
                        AnalysisOptions::new(),
                    )
                    .await;
                json!({
                    "status": "ok",
                    "package": package_summary(&ctx),
                    "report": scanner_report_value(&report)?,
                })
            }
            tool_name::TEST_SCANNER_REPORT => {
                let args = parse_args::<TestScannerArgs>(&arguments)?;
                let ctx = self.resolve_context(&args.package).map_err(tool_error)?;
                serde_json::to_value(TestScannerResponse {
                    status: "ok".to_string(),
                    package: package_summary(&ctx),
                    report: sui_test_scanner_report(&ctx, args.source_mode)
                        .map_err(|error| tool_error(error.to_string()))?,
                })
                .map_err(serialization_error)?
            }
            tool_name::PACKAGE_INSIGHTS => {
                let args = parse_args::<PackageArgs>(&arguments)?;
                let ctx = self.resolve_context(&args).map_err(tool_error)?;
                json!({
                    "status": "ok",
                    "package": package_summary(&ctx),
                    "report": sui_package_insights(&ctx)
                        .map_err(|error| tool_error(error.to_string()))?,
                })
            }
            tool_name::GRAPHS => {
                let args = parse_args::<PackageArgs>(&arguments)?;
                let ctx = self.resolve_context(&args).map_err(tool_error)?;
                let report = self
                    .run_package_analysis(
                        &ctx,
                        vec![AnalysisStage::Scan, AnalysisStage::Graph],
                        [GraphKind::CALL, GraphKind::TYPE, GraphKind::STATE_ACCESS]
                            .into_iter()
                            .map(GraphKind::new)
                            .collect(),
                        Vec::new(),
                        AnalysisOptions::new(),
                    )
                    .await;
                let graphs = legacy_project_graphs(&report)?;
                serde_json::to_value(GraphsResponse {
                    status: "ok".to_string(),
                    package: package_summary(&ctx),
                    graphs,
                })
                .map_err(serialization_error)?
            }
            tool_name::FUNCTION_STATE_GRAPH => {
                let args = parse_args::<FunctionStateGraphArgs>(&arguments)?;
                let ctx = self.resolve_context(&args.package).map_err(tool_error)?;
                let report = self
                    .run_package_analysis(
                        &ctx,
                        vec![AnalysisStage::Scan, AnalysisStage::Graph],
                        vec![GraphKind::new(GraphKind::STATE_ACCESS)],
                        Vec::new(),
                        AnalysisOptions::from([
                            ("address".to_string(), json!(args.address)),
                            ("moduleName".to_string(), json!(args.module_name)),
                            ("functionName".to_string(), json!(args.function_name)),
                        ]),
                    )
                    .await;
                let graph = legacy_state_graph(&report)?;
                serde_json::to_value(FunctionStateGraphResponse {
                    status: "ok".to_string(),
                    package: package_summary(&ctx),
                    graph,
                })
                .map_err(serialization_error)?
            }
            tool_name::BYTECODE_VIEW => {
                let args = parse_args::<PackageArgs>(&arguments)?;
                let ctx = self.resolve_context(&args).map_err(tool_error)?;
                let bytecode = serde_json::from_value::<MoveBytecodePackageView>(
                    serde_json::to_value(
                        sui_bytecode_view(&ctx).map_err(|error| tool_error(error.to_string()))?,
                    )
                    .map_err(serialization_error)?,
                )
                .map_err(serialization_error)?;
                serde_json::to_value(BytecodeViewResponse {
                    status: "ok".to_string(),
                    package: package_summary(&ctx),
                    bytecode,
                })
                .map_err(serialization_error)?
            }
            tool_name::BYTECODE_DECOMPILE => {
                let args = parse_args::<PackageArgs>(&arguments)?;
                let ctx = self.resolve_context(&args).map_err(tool_error)?;
                json!({
                    "status": "ok",
                    "package": package_summary(&ctx),
                    "decompiled": sui_bytecode_decompile(&ctx)
                        .map_err(|error| tool_error(error.to_string()))?,
                })
            }
            tool_name::COMMAND => {
                let args = parse_args::<SuiCommandArgs>(&arguments)?;
                let ctx = self.resolve_context(&args.package).map_err(tool_error)?;
                let kind = SecuritySuiCommandKind::parse(&args.command_kind)
                    .map_err(|error| tool_error(error.to_string()))?;
                let security_command = build_sui_package_command(
                    &ctx,
                    &to_adapter_settings(&self.adapter_settings),
                    kind,
                    args.publish_build_env.as_deref(),
                    args.with_unpublished_dependencies.unwrap_or(false),
                )
                .map_err(|error| tool_error(error.to_string()))?;
                serde_json::to_value(
                    command::run(
                        security_command,
                        package_summary(&ctx),
                        args.timeout_ms.unwrap_or(DEFAULT_COMMAND_TIMEOUT_MS),
                    )
                    .await,
                )
                .map_err(serialization_error)?
            }
            tool_name::MOVY_FUZZ => {
                let args = parse_args::<MovyFuzzArgs>(&arguments)?;
                let ctx = self.resolve_context(&args.package).map_err(tool_error)?;
                let time_limit_seconds = args
                    .time_limit_seconds
                    .unwrap_or(DEFAULT_MOVY_FUZZ_TIME_LIMIT_SECONDS);
                let report = self
                    .run_package_analysis(
                        &ctx,
                        vec![AnalysisStage::Scan, AnalysisStage::Dynamic],
                        Vec::new(),
                        vec!["fuzzing".to_string()],
                        AnalysisOptions::from([
                            ("timeLimitSeconds".to_string(), json!(time_limit_seconds)),
                            (
                                "seed".to_string(),
                                json!(args.seed.unwrap_or(DEFAULT_MOVY_FUZZ_SEED)),
                            ),
                        ]),
                    )
                    .await;
                serde_json::to_value(dynamic_command_result(
                    &ctx,
                    "Movy fuzzing",
                    "fuzzing",
                    &report,
                ))
                .map_err(serialization_error)?
            }
            tool_name::FORMAL_VERIFY => {
                let args = parse_args::<FormalVerifyArgs>(&arguments)?;
                let ctx = self.resolve_context(&args.package).map_err(tool_error)?;
                let report = self
                    .run_package_analysis(
                        &ctx,
                        vec![AnalysisStage::Scan, AnalysisStage::Dynamic],
                        Vec::new(),
                        vec!["formalVerification".to_string()],
                        AnalysisOptions::from([
                            ("filePath".to_string(), json!(args.file_path)),
                            ("moduleName".to_string(), json!(args.module_name)),
                            (
                                "timeoutSeconds".to_string(),
                                json!(
                                    args.timeout_seconds
                                        .unwrap_or(DEFAULT_FORMAL_VERIFY_TIMEOUT_SECONDS)
                                ),
                            ),
                            ("trace".to_string(), json!(args.trace)),
                            ("keepTemp".to_string(), json!(args.keep_temp)),
                        ]),
                    )
                    .await;
                serde_json::to_value(dynamic_command_result(
                    &ctx,
                    "Sui formal verification",
                    "formalVerification",
                    &report,
                ))
                .map_err(serialization_error)?
            }
            tool_name::ANALYZE => {
                let args = parse_args::<AnalyzeArgs>(&arguments)?;
                let (target, project_root, package_path) =
                    self.resolve_analysis_target(args.target.clone()).await?;
                let mut request =
                    AnalysisRequest::safe(peregrine_analysis::ChainId::new("sui"), target);
                apply_analyze_args(&mut request, args);
                request
                    .options
                    .insert("projectRoot".to_string(), json!(project_root));
                request
                    .options
                    .insert("packagePath".to_string(), json!(package_path));
                let report = self.analysis_engine.run(request).await;
                serde_json::to_value(EngineAnalysisResponse {
                    status: "ok".to_string(),
                    report,
                })
                .map_err(serialization_error)?
            }
            name => {
                return Err(ErrorData::invalid_params(
                    format!("unknown tool `{name}`"),
                    None,
                ));
            }
        };

        bounded_structured_result(value)
    }
}

impl PeregrineMcpServer {
    async fn resolve_analysis_target(
        &self,
        target: AnalyzeTargetArgs,
    ) -> Result<(AnalysisTarget, String, String), ErrorData> {
        match target {
            AnalyzeTargetArgs::LocalPackage {
                project_root,
                package_path,
            } => {
                let package = PackageArgs {
                    project_root,
                    package_path,
                };
                let context = self.resolve_context(&package).map_err(tool_error)?;
                Ok((
                    AnalysisTarget::LocalPackage {
                        path: context.package_root.clone(),
                    },
                    context.project_root.display().to_string(),
                    context.package_path,
                ))
            }
            AnalyzeTargetArgs::OnChainPackage {
                project_root,
                network_id,
                graph_ql_url,
                package_id,
                max_dependency_depth,
                max_dependency_packages,
            } => {
                let project_root = self
                    .resolve_project_root(project_root.as_deref())
                    .map_err(tool_error)?;
                let import_root = default_import_root(&project_root, &network_id, &package_id)
                    .map_err(tool_error)?;
                let artifact = ImportEngine::new(ImportEngineConfig {
                    max_dependency_depth: max_dependency_depth.unwrap_or(3).min(16),
                    max_dependency_packages: max_dependency_packages.unwrap_or(64).clamp(1, 512),
                    build_verification: BuildVerification::Disabled,
                })
                .import_buildable_package(BuildableImportRequest {
                    network_id,
                    graph_ql_url,
                    package_id,
                    import_root,
                    generate_buildable: true,
                })
                .await
                .map_err(tool_error)?;
                Ok((
                    AnalysisTarget::LocalPackage {
                        path: artifact.buildable_root.clone(),
                    },
                    artifact.buildable_root.display().to_string(),
                    ".".to_string(),
                ))
            }
        }
    }
}

impl ServerHandler for PeregrineMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build()).with_server_info(
            Implementation::new(env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"))
                .with_title("Peregrine Sui Analysis"),
        )
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        let tools = tool_definitions()
            .into_iter()
            .map(|definition| {
                let input_schema = serde_json::from_value::<JsonObject>(definition.input_schema)
                    .map_err(serialization_error)?;
                let mut tool = Tool::new(
                    Cow::Borrowed(definition.name),
                    Cow::Borrowed(definition.description),
                    Arc::new(input_schema),
                );
                tool.annotations = Some(
                    ToolAnnotations::new()
                        .read_only(definition.read_only)
                        .destructive(definition.destructive)
                        .open_world(definition.open_world),
                );
                Ok(tool)
            })
            .collect::<Result<Vec<_>, ErrorData>>()?;
        Ok(ListToolsResult {
            tools,
            next_cursor: None,
            meta: None,
        })
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        match self.dispatch(request).await {
            Ok(result) => Ok(result),
            Err(error) => Ok(CallToolResult::structured_error(json!({
                "status": "error",
                "message": error.message,
            }))),
        }
    }
}

fn to_adapter_settings(settings: &SuiAdapterSettings) -> AdapterSettings {
    AdapterSettings {
        source: match settings.source {
            SuiAdapterSource::Bundled => AdapterSource::Bundled,
            SuiAdapterSource::System => AdapterSource::System,
        },
        cli_path: settings.cli_path.clone(),
    }
}

fn parse_args<T>(arguments: &JsonObject) -> Result<T, ErrorData>
where
    T: DeserializeOwned,
{
    serde_json::from_value(Value::Object(arguments.clone()))
        .map_err(|error| ErrorData::invalid_params(error.to_string(), None))
}

fn package_summary(ctx: &MovePackageContext) -> PackageSummary {
    PackageSummary {
        project_root: ctx.project_root.display().to_string(),
        package_root: ctx.package_root.display().to_string(),
        package_path: ctx.package_path.clone(),
        package_name: ctx.package_name.clone(),
    }
}

fn bounded_structured_result(value: Value) -> Result<CallToolResult, ErrorData> {
    let text = serde_json::to_string_pretty(&value).unwrap_or_else(|error| error.to_string());
    if text.len() > peregrine_sui_mcp_protocol::MAX_OUTPUT_BYTES {
        return Err(ErrorData::invalid_params(
            format!(
                "tool response exceeds the {} byte limit; narrow the request or use pagination",
                peregrine_sui_mcp_protocol::MAX_OUTPUT_BYTES
            ),
            None,
        ));
    }
    let mut result = CallToolResult::structured(value);
    result.content = vec![Content::text(text)];
    Ok(result)
}

fn decode_cursor(cursor: Option<&str>) -> Result<usize, ErrorData> {
    cursor
        .unwrap_or("0")
        .parse::<usize>()
        .map_err(|_| ErrorData::invalid_params("cursor must be returned by this tool", None))
}

fn workspace_output_path(project_root: &Path, output_path: &str) -> Result<PathBuf, String> {
    let output_path = Path::new(output_path.trim());
    if output_path.as_os_str().is_empty() || output_path.is_absolute() {
        return Err("output_path must be a non-empty relative path".to_string());
    }
    if output_path
        .components()
        .any(|component| matches!(component, Component::ParentDir | Component::RootDir))
    {
        return Err("output_path must remain inside project_root".to_string());
    }
    Ok(project_root.join(output_path))
}

fn tool_error(message: String) -> ErrorData {
    ErrorData::invalid_params(message, None)
}

fn serialization_error(error: serde_json::Error) -> ErrorData {
    ErrorData::internal_error(error.to_string(), None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_rejects_workspace_escape() {
        let workspace = tempfile::tempdir().expect("workspace");
        let outside = tempfile::tempdir().expect("outside");
        let server = PeregrineMcpServer::new(workspace.path().to_path_buf()).expect("MCP server");
        let error = server
            .resolve_context(&PackageArgs {
                project_root: Some(outside.path().display().to_string()),
                package_path: None,
            })
            .expect_err("workspace escape");

        assert!(error.contains("inside the MCP workspace"));
    }

    #[test]
    fn analyze_defaults_to_scan_graph_and_static_without_dynamic_execution() {
        let mut request = AnalysisRequest::safe(
            peregrine_analysis::ChainId::new("sui"),
            AnalysisTarget::LocalPackage {
                path: PathBuf::from("."),
            },
        );
        apply_analyze_args(
            &mut request,
            AnalyzeArgs {
                target: AnalyzeTargetArgs::LocalPackage {
                    project_root: None,
                    package_path: None,
                },
                stages: Vec::new(),
                graph_kinds: Vec::new(),
                plugin_ids: Vec::new(),
                dynamic_capabilities: Vec::new(),
                limits: None,
                options: AnalysisOptions::new(),
            },
        );

        assert_eq!(
            request.stages,
            vec![
                AnalysisStage::Scan,
                AnalysisStage::Graph,
                AnalysisStage::Static,
            ]
        );
        assert_eq!(request.graph_kinds, GraphKind::required());
        assert!(request.dynamic_capabilities.is_empty());
    }
}
