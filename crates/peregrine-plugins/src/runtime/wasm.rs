use super::MAX_PLUGIN_OUTPUT_BYTES;
use serde::Serialize;
use std::path::Path;
use wasmtime::{Engine, Instance, Memory, Module, Store};

pub struct WasmPluginRuntime {
    store: Store<()>,
    instance: Instance,
    memory: Memory,
}

impl WasmPluginRuntime {
    pub fn load(plugin_path: &Path) -> Result<Self, String> {
        let engine = Engine::default();
        let module = Module::from_file(&engine, plugin_path)
            .map_err(|error| format!("Could not load WASM plugin: {error}"))?;
        let mut store = Store::new(&engine, ());
        let instance = Instance::new(&mut store, &module, &[])
            .map_err(|error| format!("Could not instantiate WASM plugin: {error}"))?;
        let memory = instance
            .get_memory(&mut store, "memory")
            .ok_or_else(|| "WASM plugin must export memory.".to_string())?;

        Ok(Self {
            store,
            instance,
            memory,
        })
    }

    pub fn call_json<T: Serialize>(
        &mut self,
        function_name: &str,
        input: &T,
    ) -> Result<String, String> {
        let input = serde_json::to_vec(input)
            .map_err(|error| format!("Could not serialize plugin input: {error}"))?;
        let alloc = self
            .instance
            .get_typed_func::<i32, i32>(&mut self.store, "peregrine_alloc")
            .map_err(|error| format!("WASM plugin must export peregrine_alloc: {error}"))?;
        let function = self
            .instance
            .get_typed_func::<(i32, i32), i64>(&mut self.store, function_name)
            .map_err(|error| format!("WASM plugin must export {function_name}: {error}"))?;
        let input_ptr = alloc
            .call(&mut self.store, input.len() as i32)
            .map_err(|error| format!("Plugin allocation failed: {error}"))?;

        self.memory
            .write(&mut self.store, input_ptr as usize, &input)
            .map_err(|error| format!("Could not write plugin input memory: {error}"))?;

        let packed = function
            .call(&mut self.store, (input_ptr, input.len() as i32))
            .map_err(|error| format!("Plugin function {function_name} failed: {error}"))?;
        let (output_ptr, output_len) = unpack_plugin_result(packed)?;
        if output_len as usize > MAX_PLUGIN_OUTPUT_BYTES {
            return Err(format!(
                "Plugin output is {} bytes, above the maximum supported {} bytes.",
                output_len, MAX_PLUGIN_OUTPUT_BYTES
            ));
        }
        let mut output = vec![0_u8; output_len as usize];

        self.memory
            .read(&mut self.store, output_ptr as usize, &mut output)
            .map_err(|error| format!("Could not read plugin output memory: {error}"))?;

        String::from_utf8(output).map_err(|error| format!("Plugin output is not UTF-8: {error}"))
    }
}

fn unpack_plugin_result(result: i64) -> Result<(u32, u32), String> {
    if result < 0 {
        return Err("Plugin returned a negative result pointer/length pair.".to_string());
    }

    let result = result as u64;
    let ptr = (result >> 32) as u32;
    let len = (result & 0xffff_ffff) as u32;

    Ok((ptr, len))
}
