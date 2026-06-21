//! Model, collaboration, and reasoning popups for `ChatWidget`.
//!
//! These surfaces are tightly related because changing one often redirects
//! into another, especially while Plan mode is active.

use super::*;
use peregrine_app_server_protocol::ModelProviderAuthStrategy;
use peregrine_app_server_protocol::ModelProviderCredentialState;
use peregrine_app_server_protocol::ModelProviderEntry;
use peregrine_app_server_protocol::ModelProviderKind;
use peregrine_app_server_protocol::ModelProviderListResponse;
use peregrine_app_server_protocol::ModelProviderModelsListResponse;
use peregrine_app_server_protocol::ModelProviderWireApi;

impl ChatWidget {
    /// Open a popup to choose a quick auto model. Selecting "All models"
    /// opens the full picker with every available preset.
    pub(crate) fn open_model_popup(&mut self) {
        if !self.is_session_configured() {
            self.add_info_message(
                "Model selection is disabled until startup completes.".to_string(),
                /*hint*/ None,
            );
            return;
        }

        let presets: Vec<ModelPreset> = match self.model_catalog.try_list_models() {
            Ok(models) => models,
            Err(_) => {
                self.add_info_message(
                    "Models are being updated; please try again in a moment.".to_string(),
                    /*hint*/ None,
                );
                return;
            }
        };
        self.open_model_popup_with_presets(presets);
    }

    pub(crate) fn open_provider_popup(&mut self, providers: ModelProviderListResponse) {
        let mut items = Vec::new();
        for provider in providers.data {
            let provider_id = provider.id.clone();
            let provider_display_name = provider.display_name.clone();
            let default_model = provider.default_model.clone();
            let actions: Vec<SelectionAction> = if provider.selectable {
                if provider.kind == ModelProviderKind::Ollama {
                    vec![Box::new(move |tx| {
                        tx.send(AppEvent::OpenProviderModelPicker {
                            provider_id: provider_id.clone(),
                            provider_display_name: provider_display_name.clone(),
                        });
                    })]
                } else if provider.auth_strategy == ModelProviderAuthStrategy::AccountOrApiKey
                    && matches!(
                        provider.credential_state,
                        Some(ModelProviderCredentialState::MissingApiKey)
                            | Some(ModelProviderCredentialState::NeedsLogin)
                    )
                {
                    vec![Box::new(move |tx| {
                        tx.send(AppEvent::PromptForProviderAuthMethod {
                            provider_id: provider_id.clone(),
                            provider_display_name: provider_display_name.clone(),
                            model: default_model.clone(),
                        });
                    })]
                } else if provider.auth_strategy == ModelProviderAuthStrategy::ApiKey
                    && matches!(
                        provider.credential_state,
                        Some(ModelProviderCredentialState::MissingApiKey)
                            | Some(ModelProviderCredentialState::NeedsLogin)
                    )
                {
                    vec![Box::new(move |tx| {
                        tx.send(AppEvent::PromptForProviderApiKey {
                            provider_id: provider_id.clone(),
                            provider_display_name: provider_display_name.clone(),
                            model: default_model.clone(),
                        });
                    })]
                } else {
                    vec![Box::new(move |tx| {
                        tx.send(AppEvent::PersistProviderSelection {
                            provider_id: provider_id.clone(),
                            model: default_model.clone(),
                        });
                    })]
                }
            } else {
                Vec::new()
            };

            items.push(SelectionItem {
                name: provider.display_name.clone(),
                description: Some(provider_description_line(&provider)),
                selected_description: provider.disabled_reason.clone(),
                is_current: provider.selected,
                is_disabled: !provider.selectable,
                disabled_reason: provider.disabled_reason.clone(),
                actions,
                dismiss_on_select: provider.selectable,
                search_value: Some(format!("{} {}", provider.id, provider.display_name)),
                ..Default::default()
            });
        }

        let mut header = ColumnRenderable::new();
        header.push(Line::from("Select Provider".bold()));
        header.push(Line::from(
            "Choose the backend Peregrine should use for new sessions.".dim(),
        ));
        self.bottom_pane.show_selection_view(SelectionViewParams {
            footer_hint: Some(standard_popup_hint_line()),
            items,
            header: Box::new(header),
            ..Default::default()
        });
        self.request_redraw();
    }

    pub(crate) fn open_text_input_popup(
        &mut self,
        title: String,
        hint: String,
        initial_value: String,
        on_submit: Box<dyn Fn(String) + Send + Sync + 'static>,
    ) {
        let view = crate::agent::chatwidget::CustomPromptView::new(
            title,
            hint,
            initial_value,
            None,
            on_submit,
        );
        self.bottom_pane.show_view(Box::new(view));
    }

    pub(crate) fn open_provider_auth_method_popup(
        &mut self,
        provider_id: String,
        provider_display_name: String,
        model: Option<String>,
    ) {
        let mut items = Vec::new();

        let p_id_login = provider_id.clone();
        let m_login = model.clone();
        let login_actions: Vec<SelectionAction> = vec![Box::new(move |tx| {
            tx.send(AppEvent::BeginOAuthLogin {
                provider_id: p_id_login.clone(),
                model: m_login.clone(),
            });
        })];
        items.push(SelectionItem {
            name: "Login with Browser (OAuth)".to_string(),
            description: Some("Authenticate using your browser.".to_string()),
            actions: login_actions,
            dismiss_on_select: true,
            ..Default::default()
        });

        let p_id = provider_id.clone();
        let p_name = provider_display_name.clone();
        let m = model.clone();
        let api_key_actions: Vec<SelectionAction> = vec![Box::new(move |tx| {
            tx.send(AppEvent::PromptForProviderApiKey {
                provider_id: p_id.clone(),
                provider_display_name: p_name.clone(),
                model: m.clone(),
            });
        })];
        items.push(SelectionItem {
            name: "Enter API Key".to_string(),
            description: Some("Manually enter an API key.".to_string()),
            actions: api_key_actions,
            dismiss_on_select: true,
            ..Default::default()
        });

        let mut header = ColumnRenderable::new();
        header.push(Line::from(
            format!("Authenticate with {provider_display_name}").bold(),
        ));
        header.push(Line::from(
            "Choose how you want to provide credentials.".dim(),
        ));

        self.bottom_pane.show_selection_view(SelectionViewParams {
            footer_hint: Some(standard_popup_hint_line()),
            items,
            header: Box::new(header),
            ..Default::default()
        });
        self.request_redraw();
    }

    pub(crate) fn open_provider_model_popup(
        &mut self,
        provider_id: String,
        provider_display_name: String,
        models: ModelProviderModelsListResponse,
    ) {
        if models.data.is_empty() {
            self.add_info_message(
                format!(
                    "No installed models found for {provider_display_name}. Install one with `ollama pull <model>`."
                ),
                /*hint*/ None,
            );
            return;
        }

        let mut items = Vec::new();
        for model in models.data {
            let model_slug = model.model.clone();
            let provider_id = provider_id.clone();
            let actions: Vec<SelectionAction> = vec![Box::new(move |tx| {
                tx.send(AppEvent::PersistProviderSelection {
                    provider_id: provider_id.clone(),
                    model: Some(model_slug.clone()),
                });
            })];
            items.push(SelectionItem {
                name: model.display_name.clone(),
                description: model.description.clone(),
                is_default: model.is_default,
                actions,
                dismiss_on_select: true,
                search_value: Some(model.model.clone()),
                ..Default::default()
            });
        }

        let mut header = ColumnRenderable::new();
        header.push(Line::from(
            format!("Select {provider_display_name} Model").bold(),
        ));
        header.push(Line::from(
            "Pick an installed local model for new sessions.".dim(),
        ));
        self.bottom_pane.show_selection_view(SelectionViewParams {
            footer_hint: Some(standard_popup_hint_line()),
            items,
            header: Box::new(header),
            ..Default::default()
        });
        self.request_redraw();
    }

    fn model_menu_header(&self, title: &str, subtitle: &str) -> Box<dyn Renderable> {
        let title = title.to_string();
        let subtitle = subtitle.to_string();
        let mut header = ColumnRenderable::new();
        header.push(Line::from(title.bold()));
        header.push(Line::from(subtitle.dim()));
        if let Some(warning) = self.model_menu_warning_line() {
            header.push(warning);
        }
        Box::new(header)
    }

    fn model_menu_warning_line(&self) -> Option<Line<'static>> {
        let base_url = self.custom_openai_base_url()?;
        let warning = format!(
            "Warning: model provider base URL is overridden to {base_url}. Selecting models may not be supported or work properly."
        );
        Some(Line::from(warning.red()))
    }

    fn custom_openai_base_url(&self) -> Option<String> {
        if !self.config.model_provider.is_openai() {
            return None;
        }

        let base_url = self.config.model_provider.base_url.as_ref()?;
        let trimmed = base_url.trim();
        if trimmed.is_empty() {
            return None;
        }

        let normalized = trimmed.trim_end_matches('/');
        if normalized == DEFAULT_OPENAI_BASE_URL {
            return None;
        }

        Some(trimmed.to_string())
    }

    pub(crate) fn open_model_popup_with_presets(&mut self, presets: Vec<ModelPreset>) {
        let presets: Vec<ModelPreset> = presets
            .into_iter()
            .filter(|preset| preset.show_in_picker)
            .collect();

        let current_model = self.current_model();
        let current_label = presets
            .iter()
            .find(|preset| preset.model.as_str() == current_model)
            .map(|preset| preset.model.to_string())
            .unwrap_or_else(|| self.model_display_name().to_string());

        let (mut auto_presets, other_presets): (Vec<ModelPreset>, Vec<ModelPreset>) = presets
            .into_iter()
            .partition(|preset| Self::is_auto_model(&preset.model));

        if auto_presets.is_empty() {
            self.open_all_models_popup(other_presets);
            return;
        }

        auto_presets.sort_by_key(|preset| Self::auto_model_order(&preset.model));
        let mut items: Vec<SelectionItem> = auto_presets
            .into_iter()
            .map(|preset| {
                let description =
                    (!preset.description.is_empty()).then_some(preset.description.clone());
                let model = preset.model.clone();
                let should_prompt_plan_mode_scope = self.should_prompt_plan_mode_reasoning_scope(
                    model.as_str(),
                    Some(preset.default_reasoning_effort),
                );
                let actions = Self::model_selection_actions(
                    model.clone(),
                    Some(preset.default_reasoning_effort),
                    should_prompt_plan_mode_scope,
                );
                SelectionItem {
                    name: model.clone(),
                    description,
                    is_current: model.as_str() == current_model,
                    is_default: preset.is_default,
                    actions,
                    dismiss_on_select: true,
                    ..Default::default()
                }
            })
            .collect();

        if !other_presets.is_empty() {
            let all_models = other_presets;
            let actions: Vec<SelectionAction> = vec![Box::new(move |tx| {
                tx.send(AppEvent::OpenAllModelsPopup {
                    models: all_models.clone(),
                });
            })];

            let is_current = !items.iter().any(|item| item.is_current);
            let description = Some(format!(
                "Choose a specific model and reasoning level (current: {current_label})"
            ));

            items.push(SelectionItem {
                name: "All models".to_string(),
                description,
                is_current,
                actions,
                dismiss_on_select: true,
                ..Default::default()
            });
        }

        let header = self.model_menu_header(
            "Select Model",
            "Pick a quick auto mode or browse all models.",
        );
        self.bottom_pane.show_selection_view(SelectionViewParams {
            footer_hint: Some(standard_popup_hint_line()),
            items,
            header,
            ..Default::default()
        });
    }

    fn is_auto_model(model: &str) -> bool {
        model.starts_with("codex-auto-")
    }

    fn auto_model_order(model: &str) -> usize {
        match model {
            "codex-auto-fast" => 0,
            "codex-auto-balanced" => 1,
            "codex-auto-thorough" => 2,
            _ => 3,
        }
    }

    pub(crate) fn open_all_models_popup(&mut self, presets: Vec<ModelPreset>) {
        if presets.is_empty() {
            self.add_info_message(
                "No additional models are available right now.".to_string(),
                /*hint*/ None,
            );
            return;
        }

        let mut items: Vec<SelectionItem> = Vec::new();
        for preset in presets.into_iter() {
            let description =
                (!preset.description.is_empty()).then_some(preset.description.to_string());
            let is_current = preset.model.as_str() == self.current_model();
            let single_supported_effort = preset.supported_reasoning_efforts.len() == 1;
            let preset_for_action = preset.clone();
            let actions: Vec<SelectionAction> = vec![Box::new(move |tx| {
                let preset_for_event = preset_for_action.clone();
                tx.send(AppEvent::OpenReasoningPopup {
                    model: preset_for_event,
                });
            })];
            items.push(SelectionItem {
                name: preset.model.clone(),
                description,
                is_current,
                is_default: preset.is_default,
                actions,
                dismiss_on_select: single_supported_effort,
                dismiss_parent_on_child_accept: !single_supported_effort,
                ..Default::default()
            });
        }

        let header = self.model_menu_header(
            "Select Model and Effort",
            "Access legacy models by running peregrine -m <model_name> or in your config.toml",
        );
        self.bottom_pane.show_selection_view(SelectionViewParams {
            footer_hint: Some(self.bottom_pane.standard_popup_hint_line()),
            items,
            header,
            ..Default::default()
        });
    }

    fn model_selection_actions(
        model_for_action: String,
        effort_for_action: Option<ReasoningEffortConfig>,
        should_prompt_plan_mode_scope: bool,
    ) -> Vec<SelectionAction> {
        vec![Box::new(move |tx| {
            if should_prompt_plan_mode_scope {
                tx.send(AppEvent::OpenPlanReasoningScopePrompt {
                    model: model_for_action.clone(),
                    effort: effort_for_action,
                });
                return;
            }

            tx.send(AppEvent::UpdateModel(model_for_action.clone()));
            tx.send(AppEvent::UpdateReasoningEffort(effort_for_action));
            tx.send(AppEvent::PersistModelSelection {
                model: model_for_action.clone(),
                effort: effort_for_action,
            });
        })]
    }

    fn should_prompt_plan_mode_reasoning_scope(
        &self,
        selected_model: &str,
        selected_effort: Option<ReasoningEffortConfig>,
    ) -> bool {
        if !self.collaboration_modes_enabled()
            || self.active_mode_kind() != ModeKind::Plan
            || selected_model != self.current_model()
        {
            return false;
        }

        // Prompt whenever the selection is not a true no-op for both:
        // 1) the active Plan-mode effective reasoning, and
        // 2) the stored global defaults that would be updated by the fallback path.
        selected_effort != self.effective_reasoning_effort()
            || selected_model != self.current_collaboration_mode.model()
            || selected_effort != self.current_collaboration_mode.reasoning_effort()
    }

    pub(crate) fn open_plan_reasoning_scope_prompt(
        &mut self,
        model: String,
        effort: Option<ReasoningEffortConfig>,
    ) {
        let reasoning_phrase = match effort {
            Some(ReasoningEffortConfig::None) => "no reasoning".to_string(),
            Some(selected_effort) => {
                format!(
                    "{} reasoning",
                    Self::reasoning_effort_label(selected_effort).to_lowercase()
                )
            }
            None => "the selected reasoning".to_string(),
        };
        let plan_only_description = format!("Always use {reasoning_phrase} in Plan mode.");
        let plan_reasoning_source = if let Some(plan_override) =
            self.config.plan_mode_reasoning_effort
        {
            format!(
                "user-chosen Plan override ({})",
                Self::reasoning_effort_label(plan_override).to_lowercase()
            )
        } else if let Some(plan_mask) = collaboration_modes::plan_mask(self.model_catalog.as_ref())
        {
            match plan_mask.reasoning_effort.flatten() {
                Some(plan_effort) => format!(
                    "built-in Plan default ({})",
                    Self::reasoning_effort_label(plan_effort).to_lowercase()
                ),
                None => "built-in Plan default (no reasoning)".to_string(),
            }
        } else {
            "built-in Plan default".to_string()
        };
        let all_modes_description = format!(
            "Set the global default reasoning level and the Plan mode override. This replaces the current {plan_reasoning_source}."
        );
        let subtitle = format!("Choose where to apply {reasoning_phrase}.");

        let plan_only_actions: Vec<SelectionAction> = vec![Box::new({
            let model = model.clone();
            move |tx| {
                tx.send(AppEvent::UpdateModel(model.clone()));
                tx.send(AppEvent::UpdatePlanModeReasoningEffort(effort));
                tx.send(AppEvent::PersistPlanModeReasoningEffort(effort));
            }
        })];
        let all_modes_actions: Vec<SelectionAction> = vec![Box::new(move |tx| {
            tx.send(AppEvent::UpdateModel(model.clone()));
            tx.send(AppEvent::UpdateReasoningEffort(effort));
            tx.send(AppEvent::UpdatePlanModeReasoningEffort(effort));
            tx.send(AppEvent::PersistPlanModeReasoningEffort(effort));
            tx.send(AppEvent::PersistModelSelection {
                model: model.clone(),
                effort,
            });
        })];

        self.bottom_pane.show_selection_view(SelectionViewParams {
            title: Some(PLAN_MODE_REASONING_SCOPE_TITLE.to_string()),
            subtitle: Some(subtitle),
            footer_hint: Some(standard_popup_hint_line()),
            items: vec![
                SelectionItem {
                    name: PLAN_MODE_REASONING_SCOPE_PLAN_ONLY.to_string(),
                    description: Some(plan_only_description),
                    actions: plan_only_actions,
                    dismiss_on_select: true,
                    ..Default::default()
                },
                SelectionItem {
                    name: PLAN_MODE_REASONING_SCOPE_ALL_MODES.to_string(),
                    description: Some(all_modes_description),
                    actions: all_modes_actions,
                    dismiss_on_select: true,
                    ..Default::default()
                },
            ],
            ..Default::default()
        });
        self.notify(Notification::PlanModePrompt {
            title: PLAN_MODE_REASONING_SCOPE_TITLE.to_string(),
        });
    }

    /// Open a popup to choose the reasoning effort (stage 2) for the given model.
    pub(crate) fn open_reasoning_popup(&mut self, preset: ModelPreset) {
        let default_effort: ReasoningEffortConfig = preset.default_reasoning_effort;
        let supported = preset.supported_reasoning_efforts;
        let in_plan_mode =
            self.collaboration_modes_enabled() && self.active_mode_kind() == ModeKind::Plan;

        let warn_effort = if supported
            .iter()
            .any(|option| option.effort == ReasoningEffortConfig::XHigh)
        {
            Some(ReasoningEffortConfig::XHigh)
        } else if supported
            .iter()
            .any(|option| option.effort == ReasoningEffortConfig::High)
        {
            Some(ReasoningEffortConfig::High)
        } else {
            None
        };
        let warning_text = warn_effort.map(|effort| {
            let effort_label = Self::reasoning_effort_label(effort);
            format!("⚠ {effort_label} reasoning effort can quickly consume Plus plan rate limits.")
        });
        let warn_for_model = preset.model.starts_with("gpt-5.1-codex")
            || preset.model.starts_with("gpt-5.1-codex-max")
            || preset.model.starts_with("gpt-5.2");

        struct EffortChoice {
            stored: Option<ReasoningEffortConfig>,
            display: ReasoningEffortConfig,
        }
        let mut choices: Vec<EffortChoice> = Vec::new();
        for effort in ReasoningEffortConfig::iter() {
            if supported.iter().any(|option| option.effort == effort) {
                choices.push(EffortChoice {
                    stored: Some(effort),
                    display: effort,
                });
            }
        }
        if choices.is_empty() {
            choices.push(EffortChoice {
                stored: Some(default_effort),
                display: default_effort,
            });
        }

        if choices.len() == 1 {
            let selected_effort = choices.first().and_then(|c| c.stored);
            let selected_model = preset.model;
            if self.should_prompt_plan_mode_reasoning_scope(&selected_model, selected_effort) {
                self.app_event_tx
                    .send(AppEvent::OpenPlanReasoningScopePrompt {
                        model: selected_model,
                        effort: selected_effort,
                    });
            } else {
                self.apply_model_and_effort(selected_model, selected_effort);
            }
            return;
        }

        let default_choice: Option<ReasoningEffortConfig> = choices
            .iter()
            .any(|choice| choice.stored == Some(default_effort))
            .then_some(Some(default_effort))
            .flatten()
            .or_else(|| choices.iter().find_map(|choice| choice.stored))
            .or(Some(default_effort));

        let model_slug = preset.model.to_string();
        let is_current_model = self.current_model() == preset.model.as_str();
        let highlight_choice = if is_current_model {
            if in_plan_mode {
                self.config
                    .plan_mode_reasoning_effort
                    .or(self.effective_reasoning_effort())
            } else {
                self.effective_reasoning_effort()
            }
        } else {
            default_choice
        };
        let selection_choice = highlight_choice.or(default_choice);
        let initial_selected_idx = choices
            .iter()
            .position(|choice| choice.stored == selection_choice)
            .or_else(|| {
                selection_choice
                    .and_then(|effort| choices.iter().position(|choice| choice.display == effort))
            });
        let mut items: Vec<SelectionItem> = Vec::new();
        for choice in choices.iter() {
            let effort = choice.display;
            let mut effort_label = Self::reasoning_effort_label(effort).to_string();
            if choice.stored == default_choice {
                effort_label.push_str(" (default)");
            }

            let description = choice
                .stored
                .and_then(|effort| {
                    supported
                        .iter()
                        .find(|option| option.effort == effort)
                        .map(|option| option.description.to_string())
                })
                .filter(|text| !text.is_empty());

            let show_warning = warn_for_model && warn_effort == Some(effort);
            let selected_description = if show_warning {
                warning_text.as_ref().map(|warning_message| {
                    description.as_ref().map_or_else(
                        || warning_message.clone(),
                        |d| format!("{d}\n{warning_message}"),
                    )
                })
            } else {
                None
            };

            let model_for_action = model_slug.clone();
            let choice_effort = choice.stored;
            let should_prompt_plan_mode_scope =
                self.should_prompt_plan_mode_reasoning_scope(model_slug.as_str(), choice_effort);
            let actions: Vec<SelectionAction> = vec![Box::new(move |tx| {
                if should_prompt_plan_mode_scope {
                    tx.send(AppEvent::OpenPlanReasoningScopePrompt {
                        model: model_for_action.clone(),
                        effort: choice_effort,
                    });
                } else {
                    tx.send(AppEvent::UpdateModel(model_for_action.clone()));
                    tx.send(AppEvent::UpdateReasoningEffort(choice_effort));
                    tx.send(AppEvent::PersistModelSelection {
                        model: model_for_action.clone(),
                        effort: choice_effort,
                    });
                }
            })];

            items.push(SelectionItem {
                name: effort_label,
                description,
                selected_description,
                is_current: is_current_model && choice.stored == highlight_choice,
                actions,
                dismiss_on_select: true,
                ..Default::default()
            });
        }

        let mut header = ColumnRenderable::new();
        header.push(Line::from(
            format!("Select Reasoning Level for {model_slug}").bold(),
        ));

        self.bottom_pane.show_selection_view(SelectionViewParams {
            header: Box::new(header),
            footer_hint: Some(standard_popup_hint_line()),
            items,
            initial_selected_idx,
            ..Default::default()
        });
    }

    pub(super) fn reasoning_effort_label(effort: ReasoningEffortConfig) -> &'static str {
        match effort {
            ReasoningEffortConfig::None => "None",
            ReasoningEffortConfig::Minimal => "Minimal",
            ReasoningEffortConfig::Low => "Low",
            ReasoningEffortConfig::Medium => "Medium",
            ReasoningEffortConfig::High => "High",
            ReasoningEffortConfig::XHigh => "Extra high",
        }
    }

    pub(super) fn apply_model_and_effort_without_persist(
        &self,
        model: String,
        effort: Option<ReasoningEffortConfig>,
    ) {
        self.app_event_tx.send(AppEvent::UpdateModel(model));
        self.app_event_tx
            .send(AppEvent::UpdateReasoningEffort(effort));
    }

    fn apply_model_and_effort(&self, model: String, effort: Option<ReasoningEffortConfig>) {
        self.apply_model_and_effort_without_persist(model.clone(), effort);
        self.app_event_tx
            .send(AppEvent::PersistModelSelection { model, effort });
    }

    pub(crate) fn provider_setup_warning(provider: &ModelProviderEntry) -> Option<String> {
        provider_setup_warning(provider)
    }
}

fn provider_description_line(provider: &ModelProviderEntry) -> String {
    let auth = match provider.auth_strategy {
        ModelProviderAuthStrategy::AccountOrApiKey => "account/API key",
        ModelProviderAuthStrategy::ApiKey => "API key",
        ModelProviderAuthStrategy::Aws => "AWS auth",
        ModelProviderAuthStrategy::None => "no auth",
        ModelProviderAuthStrategy::External => "external auth",
        ModelProviderAuthStrategy::Unsupported => "unsupported auth",
    };
    let wire = match provider.wire_api {
        ModelProviderWireApi::Responses => "Responses",
        ModelProviderWireApi::ChatCompletions => "Chat Completions",
        ModelProviderWireApi::AnthropicMessages => "Anthropic Messages",
    };
    let mut description = format!("{} ({wire}, {auth})", provider.description);
    if let Some(model) = provider.default_model.as_deref() {
        description.push_str(" default model: ");
        description.push_str(model);
    }
    if let Some(setup_hint) = provider.setup_hint.as_deref() {
        description.push_str(" setup: ");
        description.push_str(setup_hint);
    }
    description
}

pub(crate) fn provider_setup_warning(provider: &ModelProviderEntry) -> Option<String> {
    match provider.credential_state {
        Some(ModelProviderCredentialState::MissingApiKey) => {
            provider.setup_hint.as_ref().map(|hint| {
                format!(
                    "{} is selected, but its API key is not configured. {hint}",
                    provider.display_name
                )
            })
        }
        Some(ModelProviderCredentialState::NeedsLogin) => {
            provider.setup_hint.as_ref().map(|hint| {
                format!(
                    "{} is selected, but authentication is not configured. {hint}",
                    provider.display_name
                )
            })
        }
        _ => None,
    }
}
