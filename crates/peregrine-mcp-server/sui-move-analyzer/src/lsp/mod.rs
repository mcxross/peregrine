mod framing;
mod session;

use peregrine_sui_move_analyzer::MoveAnalyzerAdapter;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::sync::Mutex;

pub use session::LspSession;

const MAX_SESSIONS: usize = 4;

pub struct LspManager {
    adapter: MoveAnalyzerAdapter,
    sessions: Mutex<BTreeMap<PathBuf, Arc<LspSession>>>,
}

impl LspManager {
    pub fn new(adapter: MoveAnalyzerAdapter) -> Self {
        Self {
            adapter,
            sessions: Mutex::new(BTreeMap::new()),
        }
    }

    pub async fn session(&self, root: &Path) -> Result<Arc<LspSession>, String> {
        if let Some(session) = self.sessions.lock().await.get(root).cloned() {
            if session.is_alive().await {
                return Ok(session);
            }
            self.sessions.lock().await.remove(root);
        }

        let command = self
            .adapter
            .server_command()
            .map_err(|error| error.to_string())?;
        let session = LspSession::start(root.to_path_buf(), command).await?;
        let evicted = {
            let mut sessions = self.sessions.lock().await;
            let evicted = (sessions.len() >= MAX_SESSIONS)
                .then(|| {
                    sessions
                        .first_key_value()
                        .map(|(path, session)| (path.clone(), Arc::clone(session)))
                })
                .flatten();
            if let Some((path, _)) = &evicted {
                sessions.remove(path);
            }
            sessions.insert(root.to_path_buf(), Arc::clone(&session));
            evicted.map(|(_, session)| session)
        };
        if let Some(evicted) = evicted {
            evicted.shutdown().await;
        }
        Ok(session)
    }
}
