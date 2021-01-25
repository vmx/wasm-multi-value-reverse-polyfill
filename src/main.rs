// SPDX-License-Identifier: MIT OR Apache-2.0
// This code is heavily based on
// https://github.com/rustwasm/wasm-bindgen/blob/906fa91cb834e59f75b0bfa72e4b49e55f51c9de/crates/cli-support/src/multivalue.rs

use std::{env, fs, process};

use walrus::{ExportId, ExportItem, FunctionId, Module, ValType};

/// Returns the input path and the transformations.
///
/// The transformations are a list with a tuple containing the function name and the return types.
///
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

/// Returns the export and function IDs.
fn get_ids_by_name(module: &Module, function_name: &str) -> (ExportId, FunctionId) {
    let export = module
        .exports
        .iter()
        .find(|&exp| exp.name == function_name)
        .expect(&format!(
            "cannot find function with name `{}`",
            function_name
        ));

    match export.item {
        ExportItem::Function(function_id) => (export.id(), function_id),
        _ => panic!("item is not a function"),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        println!(
            "Usage: {} wasm-file 'function1 i32 i32' 'function2 f32 f64'",
            args[0]
        );
        process::exit(1);
    }
    let (input_path, transformations) = parse_args(&args[1..]);

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
        .on_parse(wit_walrus::on_parse)
        .parse(&wasm)
        .expect("failed to parse input file as wasm");

    let shadow_stack_pointer = wasm_bindgen_wasm_conventions::get_shadow_stack_pointer(&module)
        .expect("cannot get shadow stack pointer");
    let memory = wasm_bindgen_wasm_conventions::get_memory(&module).expect("cannot get memory");

    let to_xform: Vec<(FunctionId, usize, Vec<ValType>)> = transformations
        .iter()
        .map(|(function_name, result_types)| {
            println!(
                "Make `{}` function return `{:?}`.",
                function_name, result_types
            );
            let (_export_id, function_id) = get_ids_by_name(&module, function_name);
            (function_id, 0, result_types.to_vec())
        })
        .collect();
    let export_ids: Vec<ExportId> = transformations
        .iter()
        .map(|(function_name, _)| {
            let (export_id, _function_id) = get_ids_by_name(&module, function_name);
            export_id
        })
        .collect();

    let wrappers = wasm_bindgen_multi_value_xform::run(
        &mut module,
        memory,
        shadow_stack_pointer,
        &to_xform[..],
    )
    .expect("cannot create multi-value wrapper");

    for (export_id, id) in export_ids.into_iter().zip(wrappers) {
        let mut_export = module.exports.get_mut(export_id);
        mut_export.item = id.into();
    }

    let output_bytes = module.emit_wasm();
    let output_path = [&input_path, ".multivalue.wasm"].concat();
    fs::write(&output_path, output_bytes).expect(&format!("failed to write `{}`", output_path));
    Ok(())
}
