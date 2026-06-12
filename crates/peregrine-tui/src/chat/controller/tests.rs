use super::*;
use crate::agent::chatwidget::tests::make_chatwidget_manual;
use crate::agent::start_embedded_app_server_for_picker;
use crate::theme::ThemeName;
use ratatui::Terminal;
use ratatui::backend::TestBackend;

#[test]
fn releasing_mode_runtime_keeps_handed_off_app_server_alive() -> color_eyre::Result<()> {
    let peregrine_home = tempfile::tempdir()?;
    let shared_runtime = Arc::new(crate::build_agent_runtime()?);
    let config = shared_runtime.block_on(
        ConfigBuilder::default()
            .peregrine_home(peregrine_home.path().to_path_buf())
            .harness_overrides(ConfigOverrides {
                cwd: Some(peregrine_home.path().to_path_buf()),
                ..ConfigOverrides::default()
            })
            .build(),
    )?;
    let mut app_server = shared_runtime.block_on(start_embedded_app_server_for_picker(&config))?;

    shutdown_owned_runtime(shared_runtime.clone());

    shared_runtime.block_on(app_server.read_account())?;
    shared_runtime.block_on(app_server.shutdown())?;
    Ok(())
}

#[test]
fn embedded_chat_tick_flushes_typed_characters_in_order() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    let (chat_widget, _app_events, _ops) =
        runtime.block_on(make_chatwidget_manual(/*model_override*/ None));
    let mut controller = ChatController::default();
    controller.runtime = Some(Arc::new(runtime));
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
fn session_history_uses_active_workbench_palette() {
    let palette = ThemeName::BytecodeEmber.palette();
    let mut controller = ChatController::default();
    controller.mode = HostMode::Sessions;
    controller.sessions = vec![SessionRow {
        thread_id: ThreadId::new(),
        title: "themed session".to_string(),
        cwd: PathBuf::from("/workspace"),
        updated_at: 0,
    }];
    let mut terminal = Terminal::new(TestBackend::new(60, 8)).expect("terminal");

    terminal
        .draw(|frame| controller.render(frame, frame.area(), true, palette))
        .expect("draw");

    let buffer = terminal.backend().buffer();
    let border_style = buffer[(0, 0)].style();
    let selected_style = buffer[(3, 1)].style();
    assert_eq!(
        (border_style.fg, border_style.bg),
        (Some(palette.accent), Some(palette.bg))
    );
    assert_eq!(
        (selected_style.fg, selected_style.bg),
        (Some(palette.fg), Some(palette.selection))
    );
    assert!(selected_style.add_modifier.contains(Modifier::BOLD));
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
