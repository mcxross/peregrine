use super::{MAX_PLUGIN_OUTPUT_BYTES, PLUGIN_FREE_EXPORT};
use libloading::Library;
use serde::Serialize;
use std::{path::Path, slice};

#[repr(C)]
pub struct NativePluginBuffer {
    pub ptr: *mut u8,
    pub len: usize,
}

type NativePluginCall =
    unsafe extern "C" fn(input_ptr: *const u8, input_len: usize) -> *mut NativePluginBuffer;
type NativePluginFree = unsafe extern "C" fn(buffer: *mut NativePluginBuffer);

pub struct NativePluginRuntime {
    library: Library,
}

impl NativePluginRuntime {
    pub fn load(plugin_path: &Path) -> Result<Self, String> {
        let library = unsafe { Library::new(plugin_path) }.map_err(|error| {
            format!(
                "Could not load native plugin {}: {error}",
                plugin_path.display()
            )
        })?;

        unsafe { load_native_free(&library)? };

        Ok(Self { library })
    }

    pub fn call_json<T: Serialize>(
        &self,
        function_name: &str,
        input: &T,
    ) -> Result<String, String> {
        let input = serde_json::to_vec(input)
            .map_err(|error| format!("Could not serialize plugin input: {error}"))?;
        let function = unsafe { load_native_call(&self.library, function_name)? };
        let free = unsafe { load_native_free(&self.library)? };
        let output = unsafe { function(input.as_ptr(), input.len()) };
        read_native_plugin_output(output, free)
    }
}

unsafe fn load_native_call(
    library: &Library,
    function_name: &str,
) -> Result<NativePluginCall, String> {
    library
        .get::<NativePluginCall>(function_name.as_bytes())
        .map(|symbol| *symbol)
        .map_err(|error| format!("Native plugin must export {function_name}: {error}"))
}

unsafe fn load_native_free(library: &Library) -> Result<NativePluginFree, String> {
    library
        .get::<NativePluginFree>(PLUGIN_FREE_EXPORT.as_bytes())
        .map(|symbol| *symbol)
        .map_err(|error| format!("Native plugin must export {PLUGIN_FREE_EXPORT}: {error}"))
}

fn read_native_plugin_output(
    output: *mut NativePluginBuffer,
    free: NativePluginFree,
) -> Result<String, String> {
    if output.is_null() {
        return Err("Native plugin returned a null output buffer.".to_string());
    }

    let result = unsafe {
        let buffer = &*output;
        if buffer.len > MAX_PLUGIN_OUTPUT_BYTES {
            free(output);
            return Err(format!(
                "Plugin output is {} bytes, above the maximum supported {} bytes.",
                buffer.len, MAX_PLUGIN_OUTPUT_BYTES
            ));
        }

        if buffer.ptr.is_null() && buffer.len > 0 {
            free(output);
            return Err(
                "Native plugin returned a null output pointer with non-zero length.".to_string(),
            );
        }

        let bytes = if buffer.len == 0 {
            Vec::new()
        } else {
            slice::from_raw_parts(buffer.ptr as *const u8, buffer.len).to_vec()
        };
        free(output);
        bytes
    };

    String::from_utf8(result).map_err(|error| format!("Plugin output is not UTF-8: {error}"))
}

#[cfg(test)]
mod tests {
    use crate::{
        PluginManifestInput, PluginRuntime, PLUGIN_MANIFEST_EXPORT, PLUGIN_SCHEMA_VERSION,
    };
    use serde_json::Value;
    use std::{fs, path::PathBuf, process::Command};
    use tempfile::TempDir;

    #[test]
    fn native_runtime_calls_json_exports() {
        let temp = tempfile::tempdir().expect("tempdir");
        let plugin_path =
            compile_native_fixture(&temp, native_manifest_fixture(), "native_manifest");
        let mut runtime = PluginRuntime::load_from_path(&plugin_path).expect("load native plugin");
        let output = runtime
            .call_json(
                PLUGIN_MANIFEST_EXPORT,
                &PluginManifestInput {
                    schema_version: PLUGIN_SCHEMA_VERSION,
                },
            )
            .expect("call manifest");
        let output = serde_json::from_str::<Value>(&output).expect("json output");

        assert_eq!(output["schemaVersion"], PLUGIN_SCHEMA_VERSION);
        assert_eq!(output["pluginId"], "native-runtime-fixture");
    }

    fn compile_native_fixture(temp: &TempDir, source: &str, stem: &str) -> PathBuf {
        let source_path = temp.path().join(format!("{stem}.rs"));
        let output_path = temp.path().join(format!(
            "{}{stem}.{}",
            std::env::consts::DLL_PREFIX,
            std::env::consts::DLL_EXTENSION
        ));
        fs::write(&source_path, source).expect("fixture source");

        let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
        let output = Command::new(rustc)
            .arg("--crate-type")
            .arg("cdylib")
            .arg("--edition=2021")
            .arg(&source_path)
            .arg("-o")
            .arg(&output_path)
            .output()
            .expect("run rustc");

        assert!(
            output.status.success(),
            "native plugin fixture did not compile\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );

        output_path
    }

    fn native_manifest_fixture() -> &'static str {
        r##"
#[repr(C)]
pub struct PeregrinePluginBuffer {
    pub ptr: *mut u8,
    pub len: usize,
}

fn output(source: &str) -> *mut PeregrinePluginBuffer {
    let mut bytes = source.as_bytes().to_vec();
    let ptr = bytes.as_mut_ptr();
    let len = bytes.len();
    std::mem::forget(bytes);
    Box::into_raw(Box::new(PeregrinePluginBuffer { ptr, len }))
}

#[no_mangle]
pub extern "C" fn peregrine_plugin_manifest(
    _input_ptr: *const u8,
    _input_len: usize,
) -> *mut PeregrinePluginBuffer {
    output(r#"{"schemaVersion":1,"pluginId":"native-runtime-fixture","version":"0.1.0"}"#)
}

#[no_mangle]
pub unsafe extern "C" fn peregrine_plugin_free(buffer: *mut PeregrinePluginBuffer) {
    if buffer.is_null() {
        return;
    }

    let buffer = Box::from_raw(buffer);
    if !buffer.ptr.is_null() && buffer.len > 0 {
        let _ = Vec::from_raw_parts(buffer.ptr, buffer.len, buffer.len);
    }
}
"##
    }
}
