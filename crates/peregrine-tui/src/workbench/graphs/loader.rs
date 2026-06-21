use super::text::{
    filter_type_graph, graph_step_document, render_type_graph_text, text_graph_output_args,
};
use crate::session;
use crate::sui::args::{CallGraphArgs, CfgArgs};
use crate::sui::project::{bytecode_targets, resolve_context};
use crate::sui::runners::{run_call_graph, run_cfg};
use crate::workbench::GraphTab;
use crate::workbench::prelude::*;
use peregrine_sui_mcp_protocol::{
    GraphsResponse, PackageArgs as McpPackageArgs, tool_name,
};
use std::ffi::OsStr;
use std::path::Path;
use std::sync::mpsc;
use std::thread;

impl App {
    pub(crate) fn ensure_graph_tab(&mut self, tab: GraphTab) {
        match self.graphs.get(tab) {
            Some(GraphPane::Ready(_)) => {
                self.status = format!("{} already loaded", tab.title());
                return;
            }
            Some(GraphPane::Loading) => {
                self.status = format!("{} is already loading", tab.title());
                return;
            }
            Some(GraphPane::Empty | GraphPane::Message(_)) | None => {}
        }

        if let Some((loading_tab, _)) = &self.graph_loader_rx {
            self.status = format!("{} is still loading", loading_tab.title());
            return;
        }

        let context = match self.current_graph_context() {
            Ok(context) => context,
            Err(message) => {
                self.status = format!("{} failed", tab.title());
                self.graphs.set_message(tab, message);
                return;
            }
        };
        let (tx, rx) = mpsc::channel();
        match thread::Builder::new()
            .name(format!("peregrine-graph-{}", tab.title().replace(' ', "-")))
            .spawn(move || {
                let result = Self::load_graph_document(tab, &context);
                let _ = tx.send(GraphLoadResult { tab, result });
            }) {
            Ok(_) => {
                self.graph_loader_rx = Some((tab, rx));
                self.status = format!("Loading {}", tab.title());
                self.graphs.set_loading(tab);
            }
            Err(error) => {
                let message = format!("Could not start {} loader: {error}", tab.title());
                self.status = format!("{} failed", tab.title());
                self.graphs.set_message(tab, message);
            }
        }
    }

    pub(crate) fn drain_graph_loader(&mut self) {
        let event = match self.graph_loader_rx.as_ref() {
            Some((_, rx)) => match rx.try_recv() {
                Ok(result) => Some(Ok(result)),
                Err(mpsc::TryRecvError::Empty) => None,
                Err(mpsc::TryRecvError::Disconnected) => Some(Err(
                    "Graph loader stopped before returning a result.".to_string(),
                )),
            },
            None => None,
        };

        match event {
            Some(Ok(result)) => {
                self.graph_loader_rx = None;
                if !matches!(self.graphs.get(result.tab), Some(GraphPane::Loading)) {
                    return;
                }
                match result.result {
                    Ok(document) => {
                        self.status = format!("Loaded {}", document.title);
                        self.graphs.set_ready(result.tab, document);
                    }
                    Err(message) => {
                        self.status = format!("{} failed", result.tab.title());
                        self.graphs.set_message(result.tab, message);
                    }
                }
            }
            Some(Err(message)) => {
                let Some((tab, _)) = self.graph_loader_rx.take() else {
                    return;
                };
                self.status = format!("{} failed", tab.title());
                self.graphs.set_message(tab, message);
            }
            None => {}
        }
    }

    pub(crate) fn load_graph_document(
        tab: GraphTab,
        context: &WorkbenchGraphContext,
    ) -> Result<GraphDocument, String> {
        match tab {
            GraphTab::Cfg => Self::load_cfg_graph_document(context),
            GraphTab::CallGraph => Self::load_call_graph_document(context),
            GraphTab::TypeGraph => Self::load_type_graph_document(context),
        }
    }

    pub(crate) fn current_graph_context(&self) -> Result<WorkbenchGraphContext, String> {
        if self.editor.dirty {
            return Err("Save the current file before loading graph views.".to_string());
        }

        let project_root = self.explorer.root.clone();
        let source_hint = self
            .editor
            .path
            .as_ref()
            .cloned()
            .or_else(|| self.explorer.selected_path().map(Path::to_path_buf));
        let package_root = source_hint
            .as_deref()
            .and_then(|path| nearest_move_package_root(path, &project_root))
            .or_else(|| nearest_move_package_root(&project_root, &project_root))
            .ok_or_else(|| {
                "Open or select a Move source file inside a package with Move.toml.".to_string()
            })?;
        let package_path = relative_path_label(&project_root, &package_root);
        let context =
            resolve_context(&project_root, &package_path).map_err(|error| error.message)?;
        let file = source_hint
            .as_ref()
            .filter(|path| path.is_file() && path.extension() == Some(OsStr::new("move")))
            .and_then(|path| path.strip_prefix(&project_root).ok())
            .map(normalized_path_string);
        let module_filters = match file.as_deref() {
            Some(file) => bytecode_targets(&context, None, Some(file))
                .map_err(|error| error.message)?
                .into_iter()
                .map(|target| target.module_name)
                .collect(),
            None => Vec::new(),
        };

        Ok(WorkbenchGraphContext {
            context,
            module_filters,
        })
    }

    pub(crate) fn load_cfg_graph_document(
        graph_context: &WorkbenchGraphContext,
    ) -> Result<GraphDocument, String> {
        let module = if graph_context.module_filters.len() == 1 {
            graph_context.module_filters.first().cloned()
        } else {
            None
        };
        let args = CfgArgs {
            module,
            function: None,
            output: text_graph_output_args(),
        };

        graph_step_document(GraphTab::Cfg, run_cfg(&graph_context.context, &args))
    }

    pub(crate) fn load_call_graph_document(
        graph_context: &WorkbenchGraphContext,
    ) -> Result<GraphDocument, String> {
        let args = CallGraphArgs {
            modules: graph_context.module_filters.clone(),
            include_external: false,
            output: text_graph_output_args(),
        };

        graph_step_document(
            GraphTab::CallGraph,
            run_call_graph(&graph_context.context, &args),
        )
    }

    pub(crate) fn load_type_graph_document(
        graph_context: &WorkbenchGraphContext,
    ) -> Result<GraphDocument, String> {
        let response = session::McpToolClient::call_blocking::<_, GraphsResponse>(
            &graph_context.context.project_root,
            tool_name::GRAPHS,
            &McpPackageArgs {
                project_root: Some(graph_context.context.project_root.display().to_string()),
                package_path: Some(graph_context.context.package_path.clone()),
                unbounded: true,
            },
        )?;
        let graph = response.graphs.type_graph;
        let graph = filter_type_graph(graph, &graph_context.module_filters);

        if graph.nodes.is_empty() {
            return Err("No type graph nodes matched the requested target.".to_string());
        }

        Ok(GraphDocument::new(
            GraphTab::TypeGraph.title(),
            render_type_graph_text(&graph),
        ))
    }
}
