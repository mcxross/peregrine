# Plugin System

Peregrine has a system-wide plugin foundation in `peregrine-plugins`. The shared
registry is independent of static analysis, dynamic analysis, the CLI, and the
GUI, so new plugin domains can use the same install, enable, disable, remove,
and runtime primitives.

Installed plugins live under `PEREGRINE_CONFIG_DIR` when it is set; otherwise
Peregrine uses the OS app config directory for `xyz.mcxross.peregrine`.

```text
plugins.json
plugins/<kind>/<plugin_id>/<version>/<checksum>.<wasm|dylib|so|dll>
```

Plugin kinds are extensible strings. Current built-in helpers include
`static_analysis` and `dynamic_analysis`; domain crates validate their own
manifest schema before registering a plugin globally.

## Runtime Model

Peregrine supports two plugin runtimes:

- WASM plugins export `memory`, `peregrine_alloc(len: i32) -> i32`, and
  domain-specific JSON functions such as `peregrine_plugin_manifest` and
  `peregrine_analyze`. Returned values pack `(ptr, len)` into an `i64`, with
  the pointer in the high 32 bits and length in the low 32 bits.
- Native plugins are platform dynamic libraries (`.dylib`, `.so`, or `.dll`)
  that export JSON functions with a C ABI and return a
  `PeregrinePluginBuffer`. The plugin owns returned memory; Peregrine copies the
  bytes and calls `peregrine_plugin_free`.

```rust
#[repr(C)]
pub struct PeregrinePluginBuffer {
    pub ptr: *mut u8,
    pub len: usize,
}

pub extern "C" fn peregrine_plugin_manifest(
    input_ptr: *const u8,
    input_len: usize,
) -> *mut PeregrinePluginBuffer;

pub extern "C" fn peregrine_analyze(
    input_ptr: *const u8,
    input_len: usize,
) -> *mut PeregrinePluginBuffer;

pub unsafe extern "C" fn peregrine_plugin_free(buffer: *mut PeregrinePluginBuffer);
```

Native plugins can be written in any language that can produce a compatible
dynamic library and C ABI.

## Static Analysis Plugins

Static analysis plugins contribute rule sets and rules to the shared
`AnalysisEngine`. The CLI and GUI both use the same engine, so bundled analyzer
rules and unbundled plugins follow the same configuration path.

Run a one-off analyzer plugin:

```sh
peregrine analyze --plugin ./target/plugin.wasm
peregrine analyze --plugin ./target/release/libplugin.dylib
```

Disable globally installed analyzer plugins for deterministic CI:

```sh
peregrine analyze --no-global-plugins
```

List discoverable bundled and plugin rule sets:

```sh
peregrine analyze --list-analyzers
```

Package rule configuration stays in `peregrine.toml`:

```toml
[analysis.plugins]
use_global = true
paths = []

[analysis.rulesets.unchecked_return]
active = true
severity = "warning"
```

The GUI Settings screen can install, enable, disable, remove, and inspect
unbundled analyzer plugins. Plugin enablement is global; rule-level
configuration remains package-local so CLI and GUI runs stay aligned.
