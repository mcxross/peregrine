use super::*;
use crate::agent::chatwidget::tests::make_chatwidget_manual;

#[test]
fn embedded_chat_tick_flushes_typed_characters_in_order() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    let (chat_widget, _app_events, _ops) =
        runtime.block_on(make_chatwidget_manual(/*model_override*/ None));
    let mut controller = ChatController::default();
    controller.runtime = Some(runtime);
    controller.mode = HostMode::Chat;
    controller.chat_widget = Some(chat_widget);
    let root = Path::new("/tmp");

    controller.handle_key(root, KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE));
    assert_eq!(
        controller
            .chat_widget
            .as_ref()
            .expect("chat widget")
            .composer_text_with_pending(),
        ""
    );

    std::thread::sleep(Duration::from_millis(20));
    controller.tick(root);

    assert_eq!(
        controller
            .chat_widget
            .as_ref()
            .expect("chat widget")
            .composer_text_with_pending(),
        "/"
    );

    for ch in ['m', 'c', 'p'] {
        controller.handle_key(root, KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE));
    }
    std::thread::sleep(Duration::from_millis(20));
    controller.tick(root);

    assert_eq!(
        controller
            .chat_widget
            .as_ref()
            .expect("chat widget")
            .composer_text_with_pending(),
        "/mcp"
    );
}

#[test]
fn mcp_inventory_request_is_forwarded_to_workbench_worker() {
    let (worker_tx, mut worker_rx) = unbounded_channel();
    let thread_id = ThreadId::new();
    let mut controller = ChatController::default();
    controller.worker_tx = Some(worker_tx);

    assert_eq!(
        controller.handle_app_event(AppEvent::FetchMcpInventory {
            detail: McpServerStatusDetail::Full,
            thread_id: Some(thread_id),
        }),
        ChatAction::None
    );

    let command = worker_rx.try_recv().expect("inventory worker command");
    let WorkerCommand::FetchMcpInventory {
        detail,
        thread_id: command_thread_id,
    } = command
    else {
        panic!("unexpected workbench chat worker command");
    };
    assert_eq!(
        (detail, command_thread_id),
        (McpServerStatusDetail::Full, Some(thread_id))
    );
}

#[test]
fn provider_picker_request_is_forwarded_to_workbench_worker() {
    let (worker_tx, mut worker_rx) = unbounded_channel();
    let mut controller = ChatController::default();
    controller.worker_tx = Some(worker_tx);

    assert_eq!(
        controller.handle_app_event(AppEvent::OpenProviderPicker),
        ChatAction::None
    );
    assert!(matches!(
        worker_rx.try_recv(),
        Ok(WorkerCommand::LoadProviders)
    ));
}

#[test]
fn provider_model_request_is_forwarded_to_workbench_worker() {
    let (worker_tx, mut worker_rx) = unbounded_channel();
    let mut controller = ChatController::default();
    controller.worker_tx = Some(worker_tx);

    assert_eq!(
        controller.handle_app_event(AppEvent::OpenProviderModelPicker {
            provider_id: "ollama".to_string(),
            provider_display_name: "Ollama".to_string(),
        }),
        ChatAction::None
    );
    let WorkerCommand::LoadProviderModels {
        provider_id,
        provider_display_name,
    } = worker_rx
        .try_recv()
        .expect("provider models worker command")
    else {
        panic!("unexpected workbench chat worker command");
    };
    assert_eq!(
        (provider_id.as_str(), provider_display_name.as_str()),
        ("ollama", "Ollama")
    );
}

#[test]
fn provider_selection_is_forwarded_to_workbench_worker() {
    let (worker_tx, mut worker_rx) = unbounded_channel();
    let mut controller = ChatController::default();
    controller.worker_tx = Some(worker_tx);

    assert_eq!(
        controller.handle_app_event(AppEvent::PersistProviderSelection {
            provider_id: "ollama".to_string(),
            model: Some("qwen3".to_string()),
        }),
        ChatAction::None
    );
    let WorkerCommand::SelectProvider { provider_id, model } = worker_rx
        .try_recv()
        .expect("provider selection worker command")
    else {
        panic!("unexpected workbench chat worker command");
    };
    assert_eq!(
        (provider_id.as_str(), model.as_deref()),
        ("ollama", Some("qwen3"))
    );
}
