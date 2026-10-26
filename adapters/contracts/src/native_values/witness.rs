use super::Plan;
use std::fmt::Write;

pub(super) fn render(plan: &Plan) -> String {
    let mut text = String::from(
        "// Generated type witness; compile against the exact frozen author.rs.\n#![allow(dead_code)]\n#[path = \"author.rs\"]\nmod madb_value_author;\n",
    );
    for (index, record) in plan.records.iter().enumerate() {
        writeln!(
            text,
            "fn madb_value_record_{index}(value: madb_value_author::{}) {{",
            record.name
        )
        .unwrap();
        for field in &record.fields {
            writeln!(
                text,
                "    let _: &{} = &value.{};",
                kind(&field.kind),
                field.name
            )
            .unwrap();
        }
        writeln!(text, "    let _ = madb_value_author::{} {{", record.name).unwrap();
        for field in &record.fields {
            let initial = match field.kind.as_str() {
                "i32" => "0_i32",
                "u32" => "0_u32",
                "String" => "::std::string::String::new()",
                _ => unreachable!(),
            };
            writeln!(text, "        {}: {initial},", field.name).unwrap();
        }
        text.push_str("    };\n}\n");
    }
    for error in &plan.errors {
        for variant in &error.variants {
            writeln!(
                text,
                "const _: () = assert!(madb_value_author::{}::{} as ::std::primitive::i32 == {}_i32);",
                error.name, variant.name, variant.value
            )
            .unwrap();
        }
    }
    for function in &plan.functions {
        let mut parameters = format!("madb_value_author::{}", function.record);
        if function.parameters.len() == 2 {
            parameters.push_str(", ::std::primitive::i32");
        }
        let result = function.error.as_ref().map_or_else(
            || format!("madb_value_author::{}", function.record),
            |error| {
                format!(
                    "::std::result::Result<madb_value_author::{}, madb_value_author::{error}>",
                    function.record
                )
            },
        );
        writeln!(
            text,
            "const _: fn({parameters}) -> {result} = madb_value_author::{};",
            function.name
        )
        .unwrap();
    }
    text
}
fn kind(kind: &str) -> &str {
    match kind {
        "String" => "::std::string::String",
        "i32" => "::std::primitive::i32",
        "u32" => "::std::primitive::u32",
        _ => unreachable!(),
    }
}
