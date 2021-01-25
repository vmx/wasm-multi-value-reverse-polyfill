// SPDX-License-Identifier: MIT OR Apache-2.0
// This code is heavily based on
// https://github.com/rustwasm/wasm-bindgen/blob/906fa91cb834e59f75b0bfa72e4b49e55f51c9de/crates/cli-support/src/multivalue.rs

use std::env;
use std::fs;
use std::process;

use walrus::{ExportItem, ValType};

/// The input parameters are expected to be a list of parameters, each of them having the form:
///
///     function_name return_value_type_1 return_value_type_2 return_value_type_n
///
/// Each separate by whitespace.
fn parse_args(args: &[String]) -> (String, Vec<(String, Vec<ValType>)>) {
    let input_path = args[0].to_string();
    let transformations = args
        .iter()
        .skip(1)
        .map(|raw_input| {
            let mut input_split: Vec<&str> = raw_input.split_whitespace().collect();
            let function_name = input_split.remove(0).to_string();
            let val_types: Vec<ValType> = input_split
                .iter()
                .map(|raw_type| match *raw_type {
                    "i32" => ValType::I32,
                    "i64" => ValType::I64,
                    "f32" => ValType::F32,
                    "f64" => ValType::F64,
                    _ => panic!(
                        "unnkown return type `{}`. It must be one of i32 |  i64 | f32 | f64.",
                        raw_type
                    ),
                })
                .collect();
            if val_types.len() < 2 {
                panic!(
                    "there must be at least two return types for function `{}`, \
                else it's not a multi-value return",
                    function_name
                );
            }
            (function_name, val_types)
        })
        .collect();
    (input_path, transformations)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.len() < 2 {
        println!(
            "Usage: {} wasm-file 'function1 i32 i32' 'function2 f32 f64'",
            args[0]
        );
        process::exit(1);
    }
    let (input_path, transformations) = parse_args(&args[1..]);
    dbg!(&input_path);
    println!("{:?}", transformations);

    let wasm = wit_text::parse_file(&input_path)
        .expect(&format!("input file `{}` can be read", input_path));
    wit_validator::validate(&wasm).expect(&format!("failed to validate `{}`", input_path));
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

        let output_bytes = module.emit_wasm();
        let output_path = [&input_path, ".multivalue.wasm"].concat();
        fs::write(&output_path, output_bytes).expect(&format!("failed to write `{}`", output_path));
    }
    Ok(())
}
