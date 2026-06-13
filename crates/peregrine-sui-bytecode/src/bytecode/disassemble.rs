fn module_imports(module: &CompiledModule) -> Vec<String> {
    let self_id = module.self_id();
    let mut imports = BTreeMap::new();

    for handle in module.module_handles() {
        let id = module.module_id_for_handle(handle);

        if id == self_id {
            continue;
        }

        imports.insert(
            format!("{}::{}", id.address().short_str_lossless(), id.name()),
            (),
        );
    }

    imports.into_keys().collect()
}

fn disassemble_module(
    module: &CompiledModule,
    source_map: Option<SourceMap>,
) -> Result<String, String> {
    let mut options = DisassemblerOptions::new();
    options.print_code = true;
    options.print_basic_blocks = true;

    let disassembler = if let Some(source_map) = source_map {
        Disassembler::new(SourceMapping::new(source_map, module), options)
    } else {
        Disassembler::new(
            SourceMapping::new_without_source_map(module, Loc::invalid())
                .map_err(|error| error.to_string())?,
            options,
        )
    };

    disassembler
        .disassemble()
        .map_err(|error| error.to_string())
}
