use super::session::BytecodeSession;
use super::types::{BytecodeLoadState, BytecodeOptions, BytecodeSelector, BytecodeTargetKey};

#[derive(Debug, Default)]
pub(crate) enum BytecodePane {
    #[default]
    Empty,
    Selecting(BytecodeSelector),
    Loading(BytecodeLoadState),
    Ready(BytecodeSession),
    Message(String),
}

impl BytecodePane {
    pub(crate) fn invalidate(&mut self) {
        *self = Self::Empty;
    }

    pub(crate) fn set_message(&mut self, message: String) {
        match self {
            Self::Message(current) if current == &message => {}
            _ => *self = Self::Message(message),
        }
    }

    pub(crate) fn ready_matches_any(&self, options: &BytecodeOptions) -> bool {
        matches!(self, Self::Ready(session) if options.contains_target_key(&session.key))
    }

    pub(crate) fn loading_matches_any(&self, options: &BytecodeOptions) -> bool {
        matches!(self, Self::Loading(load) if options.contains_target_key(&load.key))
    }

    pub(crate) fn is_loading_key(&self, key: &BytecodeTargetKey) -> bool {
        matches!(self, Self::Loading(load) if load.key == *key)
    }

    pub(crate) fn selector_matches(&self, options: &BytecodeOptions) -> bool {
        matches!(self, Self::Selecting(selector) if selector.matches(options))
    }
}
