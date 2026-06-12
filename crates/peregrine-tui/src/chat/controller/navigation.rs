use super::*;
use crossterm::event::KeyEventKind;

const SESSION_ROW_HEIGHT: usize = 2;

impl ChatController {
    pub(super) fn should_open_session_history(&self, key: KeyEvent) -> bool {
        matches!(self.mode, HostMode::Chat)
            && self.active_turn_id.is_none()
            && key.code == KeyCode::Esc
            && key.modifiers == KeyModifiers::NONE
            && key.kind == KeyEventKind::Press
            && self
                .chat_widget
                .as_ref()
                .is_none_or(ChatWidget::no_modal_or_popup_active)
    }

    pub(super) fn open_session_history(&mut self) {
        if let Some(active_thread_id) = self.active_thread_id
            && let Some(index) = self
                .sessions
                .iter()
                .position(|session| session.thread_id == active_thread_id)
        {
            self.selected_session = index;
        }
        self.reload_sessions();
        self.mode = HostMode::Sessions;
    }

    pub(crate) fn handle_left_click(&mut self, area: Rect, x: u16, y: u16) -> ChatAction {
        if !matches!(self.mode, HostMode::Sessions) {
            return ChatAction::None;
        }

        let session_area = self.chat_rows(area)[0];
        let list_area = Rect {
            x: session_area.x.saturating_add(1),
            y: session_area.y.saturating_add(1),
            width: session_area.width.saturating_sub(2),
            height: session_area.height.saturating_sub(2),
        };
        if x < list_area.x || x >= list_area.right() || y < list_area.y || y >= list_area.bottom() {
            return ChatAction::None;
        }

        let rendered_row = usize::from(y.saturating_sub(list_area.y));
        let visible_item_rows =
            usize::from(list_area.height) / SESSION_ROW_HEIGHT * SESSION_ROW_HEIGHT;
        if rendered_row >= visible_item_rows {
            return ChatAction::None;
        }

        let session_index = self
            .session_list_offset
            .saturating_add(rendered_row / SESSION_ROW_HEIGHT);
        if session_index >= self.sessions.len() {
            return ChatAction::None;
        }

        self.selected_session = session_index;
        self.resume_selected_session()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::chatwidget::tests::make_chatwidget_manual;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    #[test]
    fn escape_opens_session_history_when_no_turn_is_active() {
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

        assert_eq!(
            controller.handle_key(
                Path::new("/tmp"),
                KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
            ),
            ChatAction::None
        );
        assert_eq!(controller.mode, HostMode::Sessions);
    }

    #[test]
    fn escape_remains_routed_to_chat_while_a_turn_is_active() {
        let mut controller = ChatController::default();
        controller.mode = HostMode::Chat;
        controller.active_turn_id = Some("turn-1".to_string());
        let key = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);

        assert!(!controller.should_open_session_history(key));
        assert_eq!(controller.mode, HostMode::Chat);
    }

    #[test]
    fn clicking_either_line_resumes_the_rendered_session() {
        let thread_ids = (0..8).map(|_| ThreadId::new()).collect::<Vec<_>>();
        let area = Rect::new(0, 0, 60, 12);
        for item_line in 0..SESSION_ROW_HEIGHT {
            let sessions = thread_ids
                .iter()
                .enumerate()
                .map(|(index, thread_id)| SessionRow {
                    thread_id: *thread_id,
                    title: format!("session {index}"),
                    cwd: PathBuf::from("/tmp"),
                    updated_at: 0,
                })
                .collect();
            let (worker_tx, mut worker_rx) = unbounded_channel();
            let mut controller = ChatController::default();
            controller.mode = HostMode::Sessions;
            controller.worker_tx = Some(worker_tx);
            controller.sessions = sessions;
            controller.selected_session = 7;
            let backend = TestBackend::new(area.width, area.height);
            let mut terminal = Terminal::new(backend).expect("terminal");
            terminal
                .draw(|frame| controller.render(frame, area, true))
                .expect("draw");
            let expected_index = controller.session_list_offset;
            let expected_thread_id = thread_ids[expected_index];

            assert_eq!(
                controller.handle_left_click(
                    area,
                    area.x.saturating_add(1),
                    area.y.saturating_add(1 + item_line as u16),
                ),
                ChatAction::None
            );
            assert_eq!(controller.selected_session, expected_index);
            let WorkerCommand::Resume { thread_id } = worker_rx.try_recv().expect("resume command")
            else {
                panic!("unexpected worker command");
            };
            assert_eq!(thread_id, expected_thread_id);
        }
    }
}
