use std::{env, ffi::OsString};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MoveAnalyzerAdapterEnvironment {
    pub(crate) search_common_user_locations: bool,
    pub(crate) path: Option<OsString>,
}

impl MoveAnalyzerAdapterEnvironment {
    pub fn new() -> Self {
        Self {
            search_common_user_locations: true,
            path: env::var_os("PATH"),
        }
    }

    pub fn with_path(mut self, path: Option<OsString>) -> Self {
        self.path = path;
        self
    }

    pub fn with_common_user_locations(mut self, search_common_user_locations: bool) -> Self {
        self.search_common_user_locations = search_common_user_locations;
        self
    }
}

impl Default for MoveAnalyzerAdapterEnvironment {
    fn default() -> Self {
        Self::new()
    }
}
