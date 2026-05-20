mod native;
mod wasm;

use crate::PluginRuntimeKind;
use serde::Serialize;
use std::path::Path;

pub use native::{NativePluginBuffer, NativePluginRuntime};
pub use wasm::WasmPluginRuntime;

pub const MAX_PLUGIN_OUTPUT_BYTES: usize = 64 * 1024 * 1024;
pub const PLUGIN_MANIFEST_EXPORT: &str = "peregrine_plugin_manifest";
pub const PLUGIN_FREE_EXPORT: &str = "peregrine_plugin_free";

pub enum PluginRuntime {
    Wasm(WasmPluginRuntime),
    Native(NativePluginRuntime),
}

impl PluginRuntime {
    pub fn load(kind: PluginRuntimeKind, plugin_path: &Path) -> Result<Self, String> {
        match kind {
            PluginRuntimeKind::Wasm => WasmPluginRuntime::load(plugin_path).map(Self::Wasm),
            PluginRuntimeKind::Native => NativePluginRuntime::load(plugin_path).map(Self::Native),
        }
    }

    pub fn load_from_path(plugin_path: &Path) -> Result<Self, String> {
        Self::load(PluginRuntimeKind::from_path(plugin_path)?, plugin_path)
    }

    pub fn call_json<T: Serialize>(
        &mut self,
        function_name: &str,
        input: &T,
    ) -> Result<String, String> {
        match self {
            Self::Wasm(runtime) => runtime.call_json(function_name, input),
            Self::Native(runtime) => runtime.call_json(function_name, input),
        }
    }
}
