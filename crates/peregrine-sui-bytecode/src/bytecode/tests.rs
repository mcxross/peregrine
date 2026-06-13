#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_path(relative: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("static-analysis crate should have a workspace parent")
            .join("peregrine-sui-indexer/tests/fixtures/sui")
            .join(relative)
    }

    fn fixture_module_input(relative: &str) -> MoveModuleBytecodeInput {
        let path = fixture_path(relative);
        let name = path
            .file_stem()
            .and_then(|file_stem| file_stem.to_str())
            .expect("fixture module should have a utf-8 stem")
            .to_string();
        let bytecode = fs::read(&path)
            .unwrap_or_else(|error| panic!("could not read {}: {error}", path.display()));

        MoveModuleBytecodeInput {
            name,
            bytecode,
            disassembly: None,
        }
    }

    #[test]
    fn decompiles_function_bodies_from_bytecode() {
        let input = fixture_module_input(
            "bytecode_full_mode/build/bytecode_fixture/bytecode_modules/vault.mv",
        );
        let modules = decompile_package_bytecode_modules(&[input]).expect("decompile vault module");
        let source = &modules
            .iter()
            .find(|module| module.name == "vault")
            .expect("vault module should be returned")
            .source;

        assert!(!source.contains("Fallback interface"));
        assert!(source.contains("fun create"));
        assert!(source.contains("fun deposit"));
        assert!(source.contains("Vault {"));
    }

    #[test]
    fn decompiles_modules_as_one_package_model() {
        let inputs = vec![
            fixture_module_input("friend_function/build/friend_function/bytecode_modules/a.mv"),
            fixture_module_input("friend_function/build/friend_function/bytecode_modules/b.mv"),
        ];
        let modules = decompile_package_bytecode_modules(&inputs).expect("decompile package");
        let source = modules
            .iter()
            .map(|module| module.source.as_str())
            .collect::<Vec<_>>()
            .join("\n");

        assert_eq!(modules.len(), 2);
        assert!(!source.contains("Fallback interface"));
        assert!(source.contains("fun friend_only"));
        assert!(source.contains("fun call_friend"));
        assert!(!source.contains("abort 0"));
    }
}
