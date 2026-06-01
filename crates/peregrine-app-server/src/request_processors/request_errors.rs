use super::*;

pub(super) fn environment_selection_error_message(err: PeregrineErr) -> String {
    match err {
        PeregrineErr::InvalidRequest(message) => message,
        err => err.to_string(),
    }
}
