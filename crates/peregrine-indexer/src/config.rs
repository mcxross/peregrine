use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct IndexerConfig {
    pub db_path: Option<PathBuf>,
    pub debug_store_raw_summary_json: bool,
    pub enrich_full_mode: bool,
}

impl Default for IndexerConfig {
    fn default() -> Self {
        Self {
            db_path: None,
            debug_store_raw_summary_json: false,
            enrich_full_mode: true,
        }
    }
}
