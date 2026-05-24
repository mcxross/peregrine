use std::{
    collections::{HashMap, HashSet},
    io::Write,
    path::PathBuf,
    process::Child,
    sync::{Arc, Mutex},
};

#[derive(Default)]
pub(crate) struct IndexerCommandState {
    pub(crate) active_db_path: Mutex<Option<PathBuf>>,
    pub(crate) canceled_runs: Mutex<HashSet<String>>,
}

#[derive(Default)]
pub(crate) struct MoveAnalyzerCommandState {
    pub(crate) sessions: Mutex<HashMap<String, MoveAnalyzerSession>>,
}

pub(crate) struct MoveAnalyzerSession {
    pub(crate) child: Arc<Mutex<Child>>,
    pub(crate) root_path: String,
    pub(crate) stdin: Arc<Mutex<Box<dyn Write + Send>>>,
}
