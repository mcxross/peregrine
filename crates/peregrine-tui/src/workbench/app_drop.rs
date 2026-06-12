use super::App;
use crate::session;

impl Drop for App {
    fn drop(&mut self) {
        self.chat.shutdown();
        session::McpToolClient::shutdown_all();
    }
}
