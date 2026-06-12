use std::fs;
use std::path::Path;

use toml::Value;

#[test]
fn tui_does_not_depend_directly_on_peregrine_core() {
    let manifest_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let manifest = fs::read_to_string(&manifest_path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", manifest_path.display()));
    let manifest: Value = toml::from_str(&manifest)
        .unwrap_or_else(|err| panic!("failed to parse {}: {err}", manifest_path.display()));

    assert_no_direct_core_dependency(&manifest, "manifest");
}

fn assert_no_direct_core_dependency(value: &Value, path: &str) {
    let Some(table) = value.as_table() else {
        return;
    };

    for (key, value) in table {
        let child_path = format!("{path}.{key}");
        if key.ends_with("dependencies") {
            let dependencies = value
                .as_table()
                .unwrap_or_else(|| panic!("{child_path} must be a table"));
            for (dependency_name, dependency) in dependencies {
                let package_name = dependency
                    .as_table()
                    .and_then(|details| details.get("package"))
                    .and_then(Value::as_str)
                    .unwrap_or(dependency_name);
                assert_ne!(
                    package_name, "peregrine-core",
                    "{child_path} must not depend directly on peregrine-core"
                );
            }
        } else {
            assert_no_direct_core_dependency(value, &child_path);
        }
    }
}
