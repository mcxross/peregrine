#![allow(clippy::expect_used)]
#![allow(clippy::unwrap_used)]
use crate::artifacts::MovePackageContext;
use notify::{Event, RecursiveMode, Watcher};
use peregrine_analysis::{
    AnalysisOptions, AnalysisReport as EngineAnalysisReport, AnalysisRequest, AnalysisStage,
    AnalysisTarget, GraphKind,
};
use peregrine_analysis_engine::AnalysisEngine;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc, watch};
use tokio::task::JoinHandle;
use tokio::time::{Duration, sleep};

#[derive(Clone)]
pub struct EagerCache {
    packages: Arc<RwLock<HashMap<PathBuf, PackageState>>>,
    watch_tx: mpsc::UnboundedSender<MovePackageContext>,
}

#[derive(Clone)]
#[allow(dead_code)]
pub enum PackageState {
    Analyzing(watch::Receiver<Option<Arc<EngineAnalysisReport>>>),
    Ready(Arc<EngineAnalysisReport>),
    Failed(String),
}

impl Default for EagerCache {
    fn default() -> Self {
        panic!("EagerCache must be initialized with an AnalysisEngine");
    }
}

impl EagerCache {
    pub fn new(engine: AnalysisEngine) -> Self {
        let (watch_tx, watch_rx) = mpsc::unbounded_channel();
        let packages = Arc::new(RwLock::new(HashMap::new()));

        let packages_clone = packages.clone();
        tokio::spawn(async move {
            run_watcher(engine, packages_clone, watch_rx).await;
        });

        Self { packages, watch_tx }
    }

    /// Ask the cache to analyze and watch this package.
    pub async fn ensure_watched(&self, ctx: &MovePackageContext) {
        let root = ctx.package_root.clone();
        let mut map = self.packages.write().await;
        if !map.contains_key(&root) {
            let (_tx, rx) = watch::channel(None);
            map.insert(root.clone(), PackageState::Analyzing(rx));
            // Send to watcher
            let _ = self.watch_tx.send(ctx.clone());
        }
    }

    pub async fn get_state(&self, root: &PathBuf) -> Option<PackageState> {
        let map = self.packages.read().await;
        map.get(root).cloned()
    }
}

async fn run_watcher(
    engine: AnalysisEngine,
    packages: Arc<RwLock<HashMap<PathBuf, PackageState>>>,
    mut watch_rx: mpsc::UnboundedReceiver<MovePackageContext>,
) {
    let (event_tx, mut event_rx) = mpsc::unbounded_channel::<notify::Result<Event>>();

    let mut watcher = notify::recommended_watcher(move |res| {
        let _ = event_tx.send(res);
    })
    .expect("failed to create watcher");

    // We keep track of the known contexts so we can rebuild them when files change
    let mut watched_contexts: HashMap<PathBuf, MovePackageContext> = HashMap::new();
    let mut debounce_tasks: HashMap<PathBuf, JoinHandle<()>> = HashMap::new();

    loop {
        tokio::select! {
            Some(ctx) = watch_rx.recv() => {
                let root = ctx.package_root.clone();
                if !watched_contexts.contains_key(&root) {
                    watched_contexts.insert(root.clone(), ctx.clone());
                    let _ = watcher.watch(&root, RecursiveMode::Recursive);
                    // Trigger initial build immediately
                    trigger_build(root, ctx, engine.clone(), packages.clone());
                }
            }
            Some(res) = event_rx.recv() => {
                match res {
                    Ok(event) => {
                        // Filter for .move and Move.toml
                        let mut needs_rebuild = false;
                        for path in &event.paths {
                            if let Some(ext) = path.extension()
                                && ext == "move" {
                                    needs_rebuild = true;
                                    break;
                                }
                            if let Some(name) = path.file_name()
                                && name == "Move.toml" {
                                    needs_rebuild = true;
                                    break;
                                }
                        }

                        if needs_rebuild {
                            // Find which package_root it belongs to
                            let mut matched_root = None;
                            for (root, _) in watched_contexts.iter() {
                                if event.paths.iter().any(|p| p.starts_with(root)) {
                                    matched_root = Some(root.clone());
                                    break;
                                }
                            }

                            if let Some(root) = matched_root {
                                // Cancel previous debounce task
                                if let Some(task) = debounce_tasks.remove(&root) {
                                    task.abort();
                                }

                                let ctx = watched_contexts.get(&root).unwrap().clone();
                                let engine_clone = engine.clone();
                                let packages_clone = packages.clone();

                                // Set state to analyzing
                                let root_clone = root.clone();
                                let mut map = packages.write().await;
                                let (tx, rx) = watch::channel(None);
                                map.insert(root_clone.clone(), PackageState::Analyzing(rx));
                                drop(map);

                                // Spawn new debounce task
                                let task = tokio::spawn(async move {
                                    sleep(Duration::from_secs(2)).await;
                                    trigger_build_with_tx(root_clone, ctx, engine_clone, packages_clone, tx).await;
                                });
                                debounce_tasks.insert(root, task);
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("watch error: {e:?}");
                    }
                }
            }
        }
    }
}

fn trigger_build(
    root: PathBuf,
    ctx: MovePackageContext,
    engine: AnalysisEngine,
    packages: Arc<RwLock<HashMap<PathBuf, PackageState>>>,
) {
    tokio::spawn(async move {
        let (tx, _rx) = watch::channel(None);

        // Update the state map to use the new receiver just in case
        let mut map = packages.write().await;
        if let Some(PackageState::Analyzing(existing_rx)) = map.get_mut(&root) {
            *existing_rx = tx.subscribe();
        } else {
            map.insert(root.clone(), PackageState::Analyzing(tx.subscribe()));
        }
        drop(map);

        trigger_build_with_tx(root, ctx, engine, packages, tx).await;
    });
}

async fn trigger_build_with_tx(
    root: PathBuf,
    ctx: MovePackageContext,
    engine: AnalysisEngine,
    packages: Arc<RwLock<HashMap<PathBuf, PackageState>>>,
    tx: watch::Sender<Option<Arc<EngineAnalysisReport>>>,
) {
    let report = run_package_analysis_internal(&ctx, engine).await;
    let arc_report = Arc::new(report);
    let _ = tx.send(Some(arc_report.clone()));

    let mut map = packages.write().await;
    map.insert(root, PackageState::Ready(arc_report));
}

async fn run_package_analysis_internal(
    context: &MovePackageContext,
    engine: AnalysisEngine,
) -> EngineAnalysisReport {
    let mut options = AnalysisOptions::default();
    options.insert(
        "projectRoot".to_string(),
        serde_json::json!(context.project_root),
    );
    options.insert(
        "packagePath".to_string(),
        serde_json::json!(context.package_path),
    );
    let mut request = AnalysisRequest::safe(
        peregrine_analysis::ChainId::new("sui"),
        AnalysisTarget::LocalPackage {
            path: context.package_root.clone(),
        },
    );
    // Add missing stages we know are required for everything
    request.stages = vec![
        AnalysisStage::Scan,
        AnalysisStage::Graph,
        AnalysisStage::Static,
    ];
    request.graph_kinds = vec![
        GraphKind::new(GraphKind::CALL),
        GraphKind::new(GraphKind::TYPE),
        GraphKind::new("sui_state_access"),
    ];
    request.options = options;
    engine.run(request).await
}
