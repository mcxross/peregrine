use super::*;

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
