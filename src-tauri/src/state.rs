use std::{collections::HashSet, path::PathBuf, sync::Mutex};

#[derive(Default)]
pub(crate) struct IndexerCommandState {
    pub(crate) active_db_path: Mutex<Option<PathBuf>>,
    pub(crate) canceled_runs: Mutex<HashSet<String>>,
}
