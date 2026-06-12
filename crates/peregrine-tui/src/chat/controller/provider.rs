use super::*;

impl ChatController {
    pub(super) fn open_provider_picker(&mut self) -> ChatAction {
        if !self.send_worker(WorkerCommand::LoadProviders)
            && let Some(chat) = self.chat_widget.as_mut()
        {
            chat.add_error_message(
                "Failed to load model providers: chat worker is not ready.".to_string(),
            );
        }
        ChatAction::None
    }

    pub(super) fn open_provider_model_picker(
        &mut self,
        provider_id: String,
        provider_display_name: String,
    ) -> ChatAction {
        if !self.send_worker(WorkerCommand::LoadProviderModels {
            provider_id,
            provider_display_name,
        }) && let Some(chat) = self.chat_widget.as_mut()
        {
            chat.add_error_message(
                "Failed to load provider models: chat worker is not ready.".to_string(),
            );
        }
        ChatAction::None
    }

    pub(super) fn persist_provider_selection(
        &mut self,
        provider_id: String,
        model: Option<String>,
    ) -> ChatAction {
        if !self.send_worker(WorkerCommand::SelectProvider { provider_id, model })
            && let Some(chat) = self.chat_widget.as_mut()
        {
            chat.add_error_message(
                "Failed to save model provider: chat worker is not ready.".to_string(),
            );
        }
        ChatAction::None
    }

    pub(super) fn apply_provider_list_result(
        &mut self,
        result: std::result::Result<ModelProviderListResponse, String>,
    ) {
        let Some(chat) = self.chat_widget.as_mut() else {
            return;
        };
        match result {
            Ok(providers) => chat.open_provider_popup(providers),
            Err(err) => {
                chat.add_error_message(format!("Failed to load model providers: {err}"));
            }
        }
    }

    pub(super) fn apply_provider_models_result(
        &mut self,
        provider_id: String,
        provider_display_name: String,
        result: std::result::Result<ModelProviderModelsListResponse, String>,
    ) {
        let Some(chat) = self.chat_widget.as_mut() else {
            return;
        };
        match result {
            Ok(models) => {
                chat.open_provider_model_popup(provider_id, provider_display_name, models);
            }
            Err(err) => {
                chat.add_error_message(format!("Failed to load provider models: {err}"));
            }
        }
    }

    pub(super) fn apply_provider_selection_result(
        &mut self,
        result: std::result::Result<ModelProviderSelectResponse, String>,
    ) {
        let response = match result {
            Ok(response) => response,
            Err(err) => {
                if let Some(chat) = self.chat_widget.as_mut() {
                    chat.add_error_message(format!("Failed to save model provider: {err}"));
                }
                return;
            }
        };

        if let Some(context) = self.context.as_mut() {
            apply_provider_selection_to_config(
                &mut context.config,
                &response.selected_provider,
                response.model.as_deref(),
            );
            if let Some(model) = response.model.as_ref() {
                context.model.clone_from(model);
            }
        }

        let Some(chat) = self.chat_widget.as_mut() else {
            return;
        };
        chat.apply_model_provider_selection(response.selected_provider);
        if let Some(model) = response.model.as_deref() {
            chat.set_model(model);
        }

        let mut message = format!("Provider changed to {}", response.provider.display_name);
        if let Some(model) = response.model.as_deref() {
            message.push_str(&format!(" ({model})"));
        }
        message.push_str(". New turns will use this provider.");
        chat.add_info_message(message, /*hint*/ None);
        if let Some(warning) = ChatWidget::provider_setup_warning(&response.provider) {
            chat.add_info_message(format!("Warning: {warning}"), /*hint*/ None);
        }
    }
}

pub(super) fn apply_provider_selection_to_config(
    config: &mut Config,
    selection: &peregrine_app_server_protocol::ModelProviderSelection,
    model: Option<&str>,
) {
    config.model_provider_id.clone_from(&selection.id);
    let mut provider = config
        .model_providers
        .get(&selection.id)
        .cloned()
        .unwrap_or_else(|| codex_model_provider_info::ModelProviderInfo {
            name: selection.display_name.clone(),
            base_url: selection.runtime_base_url.clone(),
            ..Default::default()
        });
    provider.name.clone_from(&selection.display_name);
    provider.requires_openai_auth = selection.requires_openai_auth;
    config.model_provider = provider;
    config.model = model.map(str::to_string);
}
