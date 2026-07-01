mod events;
mod models;
pub(crate) mod session;
mod target;

use crate::state::AgentServerCommandState;
use models::{
    AgentServerModelListRequest, AgentServerModelListResponse, AgentServerRejectRequest,
    AgentServerResolveRequest, AgentServerStartRequest, AgentServerStartResponse,
    AgentServerStopRequest, AgentServerThreadListRequest, AgentServerThreadReadRequest,
    AgentServerTurnInterruptRequest, AgentServerTurnRequest, AgentServerTurnResponse,
};
use peregrine_app_server_protocol::{
    ClientRequest, JSONRPCErrorError, Model, ModelListParams, ModelListResponse, ModelProviderKind,
    ModelProviderListParams, ModelProviderListResponse, ModelProviderModelsListParams,
    ModelProviderModelsListResponse, RequestId, ThreadListParams, ThreadListResponse,
    ThreadReadParams, ThreadReadResponse, TurnInterruptParams, TurnInterruptResponse,
    TurnStartParams, TurnStartResponse, TurnSteerParams, TurnSteerResponse, UserInput,
};
use tauri::{AppHandle, State};

#[tauri::command]
pub(crate) async fn agent_server_start(
    app: AppHandle,
    state: State<'_, AgentServerCommandState>,
    request: AgentServerStartRequest,
) -> Result<AgentServerStartResponse, String> {
    session::start_agent_session(app, state.sessions.clone(), request).await
}

#[tauri::command]
pub(crate) async fn agent_server_turn_send(
    state: State<'_, AgentServerCommandState>,
    request: AgentServerTurnRequest,
) -> Result<AgentServerTurnResponse, String> {
    let turn_input = text_input(request.prompt);
    let session_id = request.session_id.clone();
    let (handle, request_id, thread_id, active_turn_id, cwd, workspace_roots) =
        state.sessions.prepare_turn(&request.session_id).await?;

    if let Some(expected_turn_id) = active_turn_id {
        let response: TurnSteerResponse = handle
            .request_typed(ClientRequest::TurnSteer {
                request_id,
                params: TurnSteerParams {
                    thread_id: thread_id.clone(),
                    client_user_message_id: None,
                    input: turn_input,
                    responsesapi_client_metadata: None,
                    additional_context: None,
                    expected_turn_id,
                },
            })
            .await
            .map_err(|err| err.to_string())?;
        state
            .sessions
            .set_active_turn(&session_id, response.turn_id.clone())
            .await;
        return Ok(AgentServerTurnResponse {
            thread_id,
            turn_id: response.turn_id,
        });
    }

    let response: TurnStartResponse = handle
        .request_typed(ClientRequest::TurnStart {
            request_id,
            params: TurnStartParams {
                thread_id: thread_id.clone(),
                input: turn_input,
                cwd,
                runtime_workspace_roots: workspace_roots,
                ..Default::default()
            },
        })
        .await
        .map_err(|err| err.to_string())?;
    state
        .sessions
        .set_active_turn(&session_id, response.turn.id.clone())
        .await;

    Ok(AgentServerTurnResponse {
        thread_id,
        turn_id: response.turn.id,
    })
}

#[tauri::command]
pub(crate) async fn agent_server_turn_interrupt(
    state: State<'_, AgentServerCommandState>,
    request: AgentServerTurnInterruptRequest,
) -> Result<(), String> {
    let (handle, request_id, thread_id, turn_id) = state
        .sessions
        .prepare_interrupt(&request.session_id, request.turn_id)
        .await?;
    let _: TurnInterruptResponse = handle
        .request_typed(ClientRequest::TurnInterrupt {
            request_id,
            params: TurnInterruptParams { thread_id, turn_id },
        })
        .await
        .map_err(|err| err.to_string())?;
    Ok(())
}

#[tauri::command]
pub(crate) async fn agent_server_stop(
    state: State<'_, AgentServerCommandState>,
    request: AgentServerStopRequest,
) -> Result<(), String> {
    state.sessions.stop(&request.session_id).await
}

#[tauri::command]
pub(crate) async fn agent_server_request_resolve(
    state: State<'_, AgentServerCommandState>,
    request: AgentServerResolveRequest,
) -> Result<(), String> {
    let (handle, server_request) = state
        .sessions
        .prepare_request_resolution(&request.session_id, &request.request_id)
        .await?;

    server_request
        .response_from_result(request.result.clone())
        .map_err(|err| format!("invalid server request response: {err}"))?;
    handle
        .resolve_server_request(request.request_id, request.result)
        .await
        .map_err(|err| err.to_string())?;
    state
        .sessions
        .remove_pending_request(&request.session_id, &server_request.id().clone())
        .await;
    Ok(())
}

#[tauri::command]
pub(crate) async fn agent_server_request_reject(
    state: State<'_, AgentServerCommandState>,
    request: AgentServerRejectRequest,
) -> Result<(), String> {
    let (handle, _) = state
        .sessions
        .prepare_request_resolution(&request.session_id, &request.request_id)
        .await?;
    handle
        .reject_server_request(
            request.request_id.clone(),
            JSONRPCErrorError {
                code: request.code.unwrap_or(-32000),
                message: request.message,
                data: None,
            },
        )
        .await
        .map_err(|err| err.to_string())?;
    state
        .sessions
        .remove_pending_request(&request.session_id, &request.request_id)
        .await;
    Ok(())
}

#[tauri::command]
pub(crate) async fn agent_server_model_list(
    request: AgentServerModelListRequest,
) -> Result<AgentServerModelListResponse, String> {
    let (client, _) =
        session::create_app_server_client(request.target, request.cwd, Vec::new()).await?;
    let mut models: ModelListResponse = match client
        .request_typed(ClientRequest::ModelList {
            request_id: RequestId::Integer(1),
            params: ModelListParams {
                cursor: None,
                limit: None,
                include_hidden: Some(true),
            },
        })
        .await
    {
        Ok(models) => models,
        Err(err) => {
            let _ = client.shutdown().await;
            return Err(err.to_string());
        }
    };
    let providers: ModelProviderListResponse = match client
        .request_typed(ClientRequest::ModelProviderList {
            request_id: RequestId::Integer(2),
            params: ModelProviderListParams::default(),
        })
        .await
    {
        Ok(providers) => providers,
        Err(err) => {
            let _ = client.shutdown().await;
            return Err(err.to_string());
        }
    };

    if let Some(selected) = providers
        .data
        .iter()
        .find(|p| p.id == providers.selected_provider_id)
    {
        if selected.kind == ModelProviderKind::Ollama {
            let provider_models_response = client
                .request_typed::<ModelProviderModelsListResponse>(
                    ClientRequest::ModelProviderModelsList {
                        request_id: RequestId::Integer(3),
                        params: ModelProviderModelsListParams {
                            provider_id: providers.selected_provider_id.clone(),
                        },
                    },
                )
                .await;

            if let Ok(provider_models) = provider_models_response {
                models.data = provider_models
                    .data
                    .into_iter()
                    .map(|pm| Model {
                        id: pm.id,
                        model: pm.model,
                        upgrade: None,
                        upgrade_info: None,
                        availability_nux: None,
                        display_name: pm.display_name,
                        description: pm.description.unwrap_or_default(),
                        hidden: false,
                        supported_reasoning_efforts: vec![],
                        default_reasoning_effort: Default::default(),
                        input_modalities: vec![],
                        supports_personality: false,
                        additional_speed_tiers: vec![],
                        service_tiers: vec![],
                        default_service_tier: None,
                        is_default: pm.is_default,
                    })
                    .collect();
            }
        }
    }

    client.shutdown().await.map_err(|err| err.to_string())?;

    Ok(AgentServerModelListResponse { models, providers })
}

#[tauri::command]
pub(crate) async fn agent_server_thread_list(
    request: models::AgentServerThreadListRequest,
) -> Result<ThreadListResponse, String> {
    let (client, _) =
        session::create_app_server_client(request.target, request.cwd, Vec::new()).await?;

    let response = match client
        .request_typed::<ThreadListResponse>(ClientRequest::ThreadList {
            request_id: RequestId::Integer(1),
            params: ThreadListParams {
                cursor: None,
                limit: None,
                sort_key: None,
                sort_direction: None,
                model_providers: None,
                source_kinds: None,
                archived: Some(false),
                cwd: None,
                use_state_db_only: false,
                search_term: None,
            },
        })
        .await
    {
        Ok(res) => res,
        Err(err) => {
            let _ = client.shutdown().await;
            return Err(err.to_string());
        }
    };

    client.shutdown().await.map_err(|err| err.to_string())?;

    Ok(response)
}

#[tauri::command]
pub(crate) async fn agent_server_thread_read(
    request: models::AgentServerThreadReadRequest,
) -> Result<ThreadReadResponse, String> {
    let (client, _) =
        session::create_app_server_client(request.target, request.cwd, Vec::new()).await?;

    let response = match client
        .request_typed::<ThreadReadResponse>(ClientRequest::ThreadRead {
            request_id: RequestId::Integer(1),
            params: ThreadReadParams {
                thread_id: request.thread_id,
                include_turns: false,
            },
        })
        .await
    {
        Ok(res) => res,
        Err(err) => {
            let _ = client.shutdown().await;
            return Err(err.to_string());
        }
    };

    client.shutdown().await.map_err(|err| err.to_string())?;

    Ok(response)
}

fn text_input(text: String) -> Vec<UserInput> {
    vec![UserInput::Text {
        text,
        text_elements: Vec::new(),
    }]
}

#[tauri::command]
pub(crate) async fn agent_server_model_provider_select(
    request: models::AgentServerModelProviderSelectRequest,
) -> Result<models::AgentServerModelProviderSelectResponse, String> {
    use peregrine_app_server_protocol::ModelProviderSelectParams;

    let (client, _) =
        session::create_app_server_client(request.target, request.cwd, Vec::new()).await?;

    let _response = match client
        .request_typed::<peregrine_app_server_protocol::ModelProviderSelectResponse>(
            ClientRequest::ModelProviderSelect {
                request_id: RequestId::Integer(1),
                params: ModelProviderSelectParams {
                    provider_id: request.provider_id,
                    model: request.model,
                },
            },
        )
        .await
    {
        Ok(res) => res,
        Err(err) => {
            let _ = client.shutdown().await;
            return Err(err.to_string());
        }
    };

    client.shutdown().await.map_err(|err| err.to_string())?;

    Ok(models::AgentServerModelProviderSelectResponse { success: true })
}
