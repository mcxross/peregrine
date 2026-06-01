use std::sync::Arc;

use codex_analytics::AnalyticsEventsClient;
use codex_login::AuthManager;
use peregrine_core::config::Config;

pub(crate) fn analytics_events_client_from_config(
    auth_manager: Arc<AuthManager>,
    config: &Config,
) -> AnalyticsEventsClient {
    let _ = (auth_manager, config);
    AnalyticsEventsClient::disabled()
}
