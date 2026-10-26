use super::{Context, Plan, Result};
use proc_macro2::Span;
use std::collections::{BTreeMap, BTreeSet};

const CONSTANTS: &[&str] = &[
    "MADB_VALUE_CALL_OK",
    "MADB_VALUE_CALL_BOOTSTRAP_FAILED",
    "MADB_VALUE_CALL_INTERFACE_MISMATCH",
    "MADB_VALUE_CALL_PROFILE_MISMATCH",
    "MADB_VALUE_CALL_OPERATION_FAILED",
    "MADB_VALUE_STATUS_OK",
    "MADB_VALUE_STATUS_INVALID_ARGUMENT",
    "MADB_VALUE_STATUS_INTERNAL_FAILURE",
    "MADB_VALUE_STATUS_RESOURCE_EXHAUSTED",
    "MADB_VALUE_STATUS_DOMAIN_ERROR",
    "MADB_VALUE_STATUS_UNSUPPORTED",
];
const FIXED_NAMES: &[&str] = &[
    "MadbValueStatus",
    "MadbValueCallKind",
    "MadbValueError",
    "MadbValueUtf8_64",
    "madb_value_runtime",
    "madb_value_author",
    "int8_t",
    "int16_t",
    "int32_t",
    "int64_t",
    "uint8_t",
    "uint16_t",
    "uint32_t",
    "uint64_t",
    "intptr_t",
    "uintptr_t",
    "size_t",
    "ptrdiff_t",
];

pub(super) fn is_identifier(name: &str) -> bool {
    let bytes = name.as_bytes();
    if bytes.is_empty()
        || !(bytes[0].is_ascii_alphabetic() || bytes[0] == b'_')
        || !bytes
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || *byte == b'_')
        || name == "_"
        || name.starts_with("__")
        || (bytes[0] == b'_' && bytes.get(1).is_some_and(u8::is_ascii_uppercase))
    {
        return false;
    }
    !matches!(
        name,
        "auto"
            | "break"
            | "case"
            | "char"
            | "const"
            | "continue"
            | "default"
            | "do"
            | "double"
            | "else"
            | "enum"
            | "extern"
            | "float"
            | "for"
            | "goto"
            | "if"
            | "inline"
            | "int"
            | "long"
            | "register"
            | "restrict"
            | "return"
            | "short"
            | "signed"
            | "sizeof"
            | "static"
            | "struct"
            | "switch"
            | "typedef"
            | "union"
            | "unsigned"
            | "void"
            | "volatile"
            | "while"
            | "_Alignas"
            | "_Alignof"
            | "_Atomic"
            | "_Bool"
            | "_Complex"
            | "_Generic"
            | "_Imaginary"
            | "_Noreturn"
            | "_Static_assert"
            | "_Thread_local"
    )
}

pub(super) fn identifier(context: &Context<'_>, ident: &syn::Ident) -> Result<String> {
    let name = ident.to_string();
    if !is_identifier(&name) {
        return Err(context.reject(
            ident.span(),
            "name must be a directly mapped non-reserved ASCII Rust/C identifier",
        ));
    }
    Ok(name)
}

pub(super) fn type_name(context: &Context<'_>, ident: &syn::Ident) -> Result<String> {
    let name = identifier(context, ident)?;
    if matches!(name.as_str(), "String" | "Result") {
        return Err(context.reject(
            ident.span(),
            "author types cannot shadow the standard types used by the value recipe",
        ));
    }
    Ok(name)
}

pub(super) fn check(context: &Context<'_>, plan: &Plan) -> Result<()> {
    let mut globals: BTreeMap<String, String> = BTreeMap::new();
    let mut macros = BTreeSet::new();
    for &name in FIXED_NAMES {
        globals.insert(name.into(), "fixed ABI or compiler declaration".into());
    }
    for &name in CONSTANTS {
        globals.insert(name.into(), "fixed ABI constant".into());
        macros.insert(name.to_owned());
    }
    let digest = madb_protocol::byte_digest(plan.namespace.as_bytes());
    for family in ["CLIENT", "NATIVE"] {
        let guard = format!(
            "MADB_VALUE_{family}_{}_H",
            digest[7..23].to_ascii_uppercase()
        );
        globals.insert(guard.clone(), "generated header guard".into());
        macros.insert(guard);
    }
    // The native header guards and the fixed ABI header use explicit, stable
    // names. Only actual generated names, not arbitrary business prefixes, are
    // reserved. This list is kept with the corresponding generator contract.
    globals.insert(
        "MADB_VALUE_ABI_V1_H".into(),
        "fixed value ABI header guard".into(),
    );
    macros.insert("MADB_VALUE_ABI_V1_H".into());
    let prefix = &plan.namespace;
    let mut insert = |name: String, owner: String, span: Span| -> Result<()> {
        if let Some(previous) = globals.insert(name.clone(), owner.clone()) {
            return Err(context.reject(
                span,
                format!("generated C name {name} collides between {previous} and {owner}"),
            ));
        }
        Ok(())
    };
    insert(
        format!("{prefix}_bootstrap_v1"),
        "value bootstrap".into(),
        Span::call_site(),
    )?;
    insert(
        format!("{prefix}_value_utf8_valid"),
        "ordinary-entry UTF-8 checker".into(),
        Span::call_site(),
    )?;
    for record in &plan.records {
        for (name, role) in [
            (format!("{prefix}_{}", record.name), "record"),
            (format!("{prefix}_{}Result", record.name), "ordinary result"),
            (
                format!("{prefix}_value_{}_is_zero", record.name),
                "zero-output checker",
            ),
            (
                format!("{prefix}_value_{}_valid_output", record.name),
                "output checker",
            ),
        ] {
            insert(name, format!("{role} {}", record.name), record.span)?;
        }
    }
    for error in &plan.errors {
        for variant in &error.variants {
            let name = format!("{prefix}_{}_{}", error.name, variant.name);
            insert(
                name.clone(),
                format!("error tag {}::{}", error.name, variant.name),
                variant.span,
            )?;
            macros.insert(name);
        }
    }
    for function in &plan.functions {
        insert(
            format!("{prefix}_raw_{}", function.name),
            format!("native function {}", function.name),
            function.span,
        )?;
        insert(
            format!("{prefix}_{}", function.name),
            format!("ordinary function {}", function.name),
            function.span,
        )?;
    }
    // cbindgen excludes the author declarations by their unqualified names.
    // An author name equal to a physical declaration would otherwise suppress
    // that genuine ABI declaration when the two Rust module scopes flatten.
    for (name, span) in plan
        .records
        .iter()
        .map(|item| (&item.name, item.span))
        .chain(plan.errors.iter().map(|item| (&item.name, item.span)))
        .chain(plan.functions.iter().map(|item| (&item.name, item.span)))
    {
        if globals.contains_key(name) {
            return Err(context.reject(
                span,
                "author name collides with a generated declaration after C namespace flattening",
            ));
        }
    }
    for record in &plan.records {
        for field in &record.fields {
            if macros.contains(&field.name) {
                return Err(
                    context.reject(field.span, "field name would expand as a generated C macro")
                );
            }
        }
    }
    for function in &plan.functions {
        if let Some(parameter) = function.parameters.get(1)
            && globals.contains_key(&parameter.name)
        {
            return Err(context.reject(
                parameter.span,
                "second parameter shadows a generated C declaration or macro",
            ));
        }
    }
    Ok(())
}
