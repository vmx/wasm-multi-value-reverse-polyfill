// SPDX-License-Identifier: MIT OR Apache-2.0
// This code is heavily based on
// https://github.com/rustwasm/wasm-bindgen/blob/906fa91cb834e59f75b0bfa72e4b49e55f51c9de/crates/cli-support/src/multivalue.rs

use std::fs;

use walrus::{ExportItem, ValType};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Hello, world!");

    let path = "/home/vmx/src/rust/misc/wasmbug/issue_73755.wasm";
    let wasm = wit_text::parse_file(&path).expect(&format!("input file `{}` can be read", path));
    wit_validator::validate(&wasm).expect(&format!("failed to validate `{}`", path));
    let mut module = walrus::ModuleConfig::new()
        // Skip validation of the module as LLVM's output is
        // generally already well-formed and so we won't gain much
        // from re-validating. Additionally LLVM's current output
        // for threads includes atomic instructions but doesn't
        // include shared memory, so it fails that part of
        // validation!
        .strict_validate(false)
        //.generate_dwarf(true)
        //.generate_name_section(true)
        //.generate_producers_section(true)
        .on_parse(wit_walrus::on_parse)
        .parse(&wasm)
        .expect("failed to parse input file as wasm");

    let shadow_stack_pointer = wasm_bindgen_wasm_conventions::get_shadow_stack_pointer(&module)
        .expect("cannot get shadow stack pointer");
    dbg!(&shadow_stack_pointer);
    let memory = wasm_bindgen_wasm_conventions::get_memory(&module).expect("cannot get memory");
    dbg!(&module.exports);

    //for export in module.exports.iter() {
    //    dbg!(export);
    //}
    let function_name = "test2";
    let export = {
        module
            .exports
            .iter()
            .find(|&exp| exp.name == function_name)
            .expect(&format!(
                "cannot find function with name `{}`",
                function_name
            ))
        //dbg!(&export.item);
    };

    let export_id = export.id();

    if let ExportItem::Function(function) = export.item {
        let result_types = vec![ValType::I32, ValType::I32];
        dbg!(&result_types);

        let to_xform = vec![(function, 0, result_types)];
        dbg!(&to_xform);

        //to_xform: &[(walrus::FunctionId, usize, Vec<walrus::ValType>)],

        let wrappers = wasm_bindgen_multi_value_xform::run(
            &mut module,
            memory,
            shadow_stack_pointer,
            &to_xform[..],
        )
        .expect("cannot create multi-value wrapper");
        dbg!(&wrappers);

        //let mut mut_export = module.exports.get_mut(export_id);
        //mut_export.item = wrappers[0].into();
        let mut_export = module.exports.get_mut(export_id);
        let slots = vec![mut_export];
        for (slot, id) in slots.into_iter().zip(wrappers) {
            slot.item = id.into();
        }

        let wasm_bytes = module.emit_wasm();
        let wasm_path = "/home/vmx/src/rust/misc/wasmbug/issue_73755.multivalue.wasm";
        fs::write(&wasm_path, wasm_bytes).expect(&format!("failed to write `{}`", wasm_path));
    }
    Ok(())
}
