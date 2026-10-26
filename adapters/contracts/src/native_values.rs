//! Pure, finite syntax reader for the approved natural-Rust value recipe.
//!
//! This plan is private adapter data, not a wire protocol or compiler proof.
//! The host must compile `witness` against the exact `author.rs` source bytes
//! using its frozen Rust toolchain before using the plan for checked metadata.
use madb_protocol::{FormatError, Value, canonical_json, strict_json};
use proc_macro2::{Span, TokenStream, TokenTree};
use serde_json::json;
use std::collections::BTreeSet;
use syn::{
    Attribute, Fields, FnArg, GenericArgument, Item, Pat, PathArguments, ReturnType, Type,
    Visibility, punctuated::Punctuated, spanned::Spanned, visit::Visit,
};

mod names;
mod witness;

pub const MAX_ENVELOPE_BYTES: usize = madb_protocol::MAX_DOCUMENT_BYTES;

type Result<T> = std::result::Result<T, FormatError>;

struct Context<'a> {
    source: &'a str,
    path: &'a str,
    span_offset: usize,
}
impl Context<'_> {
    fn location(&self, span: Span) -> Value {
        let offset = (span.byte_range().start + self.span_offset).min(self.source.len());
        let bytes = &self.source.as_bytes()[..offset];
        let line = bytes.iter().filter(|&&byte| byte == b'\n').count() + 1;
        let column = bytes
            .iter()
            .rposition(|&byte| byte == b'\n')
            .map_or(offset + 1, |last| offset - last);
        json!({"path": self.path, "line": line, "column": column})
    }
    fn reject(&self, span: Span, message: impl Into<String>) -> FormatError {
        let location = self.location(span);
        FormatError::new(
            "NATIVE004",
            format!("{}:{}:{}", self.path, location["line"], location["column"]),
            message,
        )
    }
    fn no_attributes(&self, attrs: &[Attribute]) -> Result<()> {
        if let Some(attr) = attrs.first() {
            return Err(self.reject(
                attr.span(),
                "attributes are unsupported at this declaration or implementation position",
            ));
        }
        Ok(())
    }
}

struct Field {
    name: String,
    kind: String,
    span: Span,
}
struct Record {
    name: String,
    fields: Vec<Field>,
    span: Span,
}
struct Variant {
    name: String,
    value: i32,
    span: Span,
}
struct ErrorEnum {
    name: String,
    variants: Vec<Variant>,
    span: Span,
}
struct Parameter {
    name: String,
    kind: String,
    span: Span,
}
struct Function {
    name: String,
    record: String,
    parameters: Vec<Parameter>,
    error: Option<String>,
    span: Span,
}
struct Plan {
    namespace: String,
    records: Vec<Record>,
    errors: Vec<ErrorEnum>,
    functions: Vec<Function>,
}

/// Parse an exact private `{source_path, source, namespace}` envelope.
///
/// This does no I/O and does not mark syntax as resolved Rust types. Output
/// locations use the original source's one-based UTF-8 byte line/column.
/// Unknown fields and duplicate JSON keys are rejected before source parsing.
pub fn read_envelope(input: &[u8]) -> Result<Vec<u8>> {
    let envelope = strict_json(input)?;
    let fields = envelope
        .as_object()
        .ok_or_else(|| FormatError::new("NATIVE001", "", "reader input must be an object"))?;
    let expected = ["namespace", "source", "source_path"];
    if fields.len() != expected.len() || expected.iter().any(|name| !fields.contains_key(*name)) {
        return Err(FormatError::new(
            "NATIVE001",
            "",
            "reader input requires exactly source_path, source and namespace",
        ));
    }
    let text = |key: &str| -> Result<&str> {
        fields[key]
            .as_str()
            .ok_or_else(|| FormatError::new("NATIVE001", format!("/{key}"), "must be a string"))
    };
    let source = text("source")?;
    let path = text("source_path")?;
    let namespace = text("namespace")?;
    if path.is_empty()
        || !path.ends_with(".rs")
        || path.starts_with('/')
        || path.contains(['\\', '\0', '\r', '\n'])
        || path.split('/').any(|part| matches!(part, "" | "." | ".."))
    {
        return Err(FormatError::new(
            "NATIVE001",
            "/source_path",
            "must be a normalized relative Rust source path",
        ));
    }
    let (parse_source, span_offset) = source
        .strip_prefix('\u{feff}')
        .map_or((source, 0), |rest| (rest, 3));
    let context = Context {
        source,
        path,
        span_offset,
    };
    if !names::is_identifier(namespace) || namespace.starts_with('_') {
        return Err(FormatError::new(
            "NATIVE004",
            "/namespace",
            "namespace must be a non-reserved ASCII C identifier",
        ));
    }
    // Unlike parse_file, parse_str does not silently remove a shebang and move
    // all subsequent spans. Only a Rust-accepted initial BOM is adjusted above.
    let file = syn::parse_str::<syn::File>(parse_source).map_err(|error| {
        context.reject(
            error.span(),
            format!("unsupported or invalid Rust source: {error}"),
        )
    })?;
    context.no_attributes(&file.attrs)?;
    let plan = parse_plan(&context, &file, namespace)?;
    names::check(&context, &plan)?;
    let output = canonical_json(&plan.value(&context))?;
    if output.len() > MAX_ENVELOPE_BYTES {
        return Err(FormatError::new(
            "NATIVE001",
            "",
            "reader plan exceeds the internal byte budget",
        ));
    }
    Ok(output)
}

fn parse_plan(context: &Context<'_>, file: &syn::File, namespace: &str) -> Result<Plan> {
    let mut plan = Plan {
        namespace: namespace.into(),
        records: Vec::new(),
        errors: Vec::new(),
        functions: Vec::new(),
    };
    let mut declared = BTreeSet::new();
    for item in &file.items {
        let ident = match item {
            Item::Struct(item) => &item.ident,
            Item::Enum(item) => &item.ident,
            Item::Fn(item) => &item.sig.ident,
            _ => return Err(context.reject(item.span(), "only explicit public records, fieldless errors and synchronous free functions are supported")),
        };
        let name = names::identifier(context, ident)?;
        if !declared.insert(name) {
            return Err(context.reject(ident.span(), "duplicate author declaration name"));
        }
        match item {
            Item::Struct(item) => plan.records.push(parse_record(context, item)?),
            Item::Enum(item) => plan.errors.push(parse_error(context, item)?),
            Item::Fn(_) => {}
            _ => unreachable!(),
        }
    }
    let records: BTreeSet<_> = plan
        .records
        .iter()
        .map(|record| record.name.as_str())
        .collect();
    let errors: BTreeSet<_> = plan
        .errors
        .iter()
        .map(|error| error.name.as_str())
        .collect();
    for item in &file.items {
        if let Item::Fn(item) = item {
            plan.functions
                .push(parse_function(context, item, &records, &errors)?);
        }
    }
    if plan.records.is_empty() || plan.functions.is_empty() {
        return Err(context.reject(
            file.span(),
            "a value module requires a public record and a supported public function",
        ));
    }
    Ok(plan)
}

fn public(context: &Context<'_>, visibility: &Visibility, span: Span) -> Result<()> {
    if !matches!(visibility, Visibility::Public(_)) {
        return Err(context.reject(span, "boundary declarations and fields must be explicitly pub so the compiler witness and generated shim can name them"));
    }
    Ok(())
}

fn nongeneric(context: &Context<'_>, generics: &syn::Generics) -> Result<()> {
    if !generics.params.is_empty() || generics.where_clause.is_some() {
        return Err(context.reject(
            generics.span(),
            "generic parameters, lifetimes and where clauses are unsupported",
        ));
    }
    Ok(())
}

fn type_attributes(
    context: &Context<'_>,
    attrs: &[Attribute],
    error_enum: bool,
    declaration: Span,
) -> Result<()> {
    let mut repr = false;
    let mut derives = BTreeSet::new();
    for attr in attrs {
        if !matches!(attr.style, syn::AttrStyle::Outer) {
            return Err(context.reject(attr.span(), "inner attributes are unsupported"));
        }
        if attr.path().is_ident("derive") {
            let paths = attr
                .parse_args_with(Punctuated::<syn::Path, syn::Token![,]>::parse_terminated)
                .map_err(|error| {
                    context.reject(
                        error.span(),
                        "derive must list the supported standard derives",
                    )
                })?;
            if paths.is_empty() {
                return Err(context.reject(attr.span(), "empty derive lists are unsupported"));
            }
            for path in paths {
                let Some(name) = path.get_ident() else {
                    return Err(context.reject(
                        path.span(),
                        "only unqualified standard derives are supported",
                    ));
                };
                let name = name.to_string();
                if !matches!(
                    name.as_str(),
                    "Clone" | "Copy" | "Debug" | "PartialEq" | "Eq"
                ) || !derives.insert(name)
                {
                    return Err(context.reject(path.span(), "unsupported or repeated derive"));
                }
            }
        } else if error_enum && attr.path().is_ident("repr") && !repr {
            let path = attr.parse_args::<syn::Path>().map_err(|error| {
                context.reject(error.span(), "error enum requires exactly repr(i32)")
            })?;
            if !path.is_ident("i32") {
                return Err(context.reject(path.span(), "error enum requires exactly repr(i32)"));
            }
            repr = true;
        } else {
            return Err(context.reject(attr.span(), "only standard Clone/Copy/Debug/PartialEq/Eq derives and error repr(i32) are supported"));
        }
    }
    if error_enum && !repr {
        return Err(context.reject(declaration, "error enum must declare repr(i32)"));
    }
    Ok(())
}

fn parse_record(context: &Context<'_>, item: &syn::ItemStruct) -> Result<Record> {
    public(context, &item.vis, item.ident.span())?;
    nongeneric(context, &item.generics)?;
    type_attributes(context, &item.attrs, false, item.ident.span())?;
    let name = names::type_name(context, &item.ident)?;
    let Fields::Named(fields) = &item.fields else {
        return Err(context.reject(item.fields.span(), "record fields must be named"));
    };
    if !(1..=16).contains(&fields.named.len()) {
        return Err(context.reject(
            item.ident.span(),
            "a record must contain 1 through 16 fields",
        ));
    }
    let mut names = BTreeSet::new();
    let mut result = Vec::new();
    for field in &fields.named {
        public(context, &field.vis, field.span())?;
        context.no_attributes(&field.attrs)?;
        let ident = field
            .ident
            .as_ref()
            .ok_or_else(|| context.reject(field.span(), "record field name is required"))?;
        let field_name = names::identifier(context, ident)?;
        if !names.insert(field_name.clone()) {
            return Err(context.reject(ident.span(), "duplicate record field"));
        }
        let kind = primitive_type(&field.ty).ok_or_else(|| {
            context.reject(
                field.ty.span(),
                "record fields support only i32, u32 or the standard String type",
            )
        })?;
        result.push(Field {
            name: field_name,
            kind: kind.into(),
            span: ident.span(),
        });
    }
    Ok(Record {
        name,
        fields: result,
        span: item.ident.span(),
    })
}

fn parse_error(context: &Context<'_>, item: &syn::ItemEnum) -> Result<ErrorEnum> {
    public(context, &item.vis, item.ident.span())?;
    nongeneric(context, &item.generics)?;
    type_attributes(context, &item.attrs, true, item.ident.span())?;
    let name = names::type_name(context, &item.ident)?;
    if item.variants.is_empty() {
        return Err(context.reject(
            item.ident.span(),
            "error enum must contain an explicit positive i32 tag",
        ));
    }
    let mut values = BTreeSet::new();
    let mut names = BTreeSet::new();
    let mut variants = Vec::new();
    for variant in &item.variants {
        context.no_attributes(&variant.attrs)?;
        if !matches!(variant.fields, Fields::Unit) {
            return Err(context.reject(variant.span(), "error variants cannot have payloads"));
        }
        let variant_name = names::identifier(context, &variant.ident)?;
        if !names.insert(variant_name.clone()) {
            return Err(context.reject(variant.ident.span(), "duplicate error variant"));
        }
        let Some((_, syn::Expr::Lit(literal))) = &variant.discriminant else {
            return Err(context.reject(
                variant.span(),
                "error tag must be an explicit positive i32 integer literal",
            ));
        };
        context.no_attributes(&literal.attrs)?;
        let syn::Lit::Int(literal) = &literal.lit else {
            return Err(context.reject(literal.span(), "error tag must be an integer literal"));
        };
        if !matches!(literal.suffix(), "" | "i32") {
            return Err(context.reject(
                literal.span(),
                "error tag literal must use i32 or inferred i32",
            ));
        }
        let value = literal
            .base10_parse::<i32>()
            .map_err(|_| context.reject(literal.span(), "error tag exceeds positive i32 range"))?;
        if value <= 0 || !values.insert(value) {
            return Err(context.reject(literal.span(), "error tags must be positive and distinct"));
        }
        variants.push(Variant {
            name: variant_name,
            value,
            span: variant.ident.span(),
        });
    }
    Ok(ErrorEnum {
        name,
        variants,
        span: item.ident.span(),
    })
}

fn path_parts(ty: &Type) -> Option<Vec<String>> {
    let Type::Path(path) = ty else {
        return None;
    };
    if path.qself.is_some()
        || path
            .path
            .segments
            .iter()
            .any(|segment| !matches!(segment.arguments, PathArguments::None))
    {
        return None;
    }
    Some(
        path.path
            .segments
            .iter()
            .map(|segment| segment.ident.to_string())
            .collect(),
    )
}

fn primitive_type(ty: &Type) -> Option<&'static str> {
    match path_parts(ty)?
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["i32"] => Some("i32"),
        ["u32"] => Some("u32"),
        ["String"] | ["std", "string", "String"] => Some("String"),
        _ => None,
    }
}

fn declared_type(ty: &Type, records: &BTreeSet<&str>) -> Option<String> {
    let Type::Path(path) = ty else {
        return None;
    };
    if path.path.leading_colon.is_some() {
        return None;
    }
    let parts = path_parts(ty)?;
    if parts.len() == 1 && records.contains(parts[0].as_str()) {
        Some(parts[0].clone())
    } else {
        None
    }
}

fn result_arguments(ty: &Type) -> Option<(&Type, &Type)> {
    let Type::Path(path) = ty else {
        return None;
    };
    let segments: Vec<_> = path.path.segments.iter().collect();
    let names: Vec<_> = segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect();
    if path.qself.is_some()
        || !matches!(
            names
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>()
                .as_slice(),
            ["Result"] | ["std", "result", "Result"]
        )
        || (segments.len() == 1 && path.path.leading_colon.is_some())
        || segments
            .iter()
            .take(segments.len() - 1)
            .any(|segment| !matches!(segment.arguments, PathArguments::None))
    {
        return None;
    }
    let PathArguments::AngleBracketed(args) = &segments.last()?.arguments else {
        return None;
    };
    let mut args = args.args.iter();
    let (GenericArgument::Type(value), GenericArgument::Type(error)) = (args.next()?, args.next()?)
    else {
        return None;
    };
    if args.next().is_some() {
        return None;
    }
    Some((value, error))
}

fn result_type(
    context: &Context<'_>,
    ty: &Type,
    record: &str,
    records: &BTreeSet<&str>,
    errors: &BTreeSet<&str>,
) -> Result<Option<String>> {
    if declared_type(ty, records).as_deref() == Some(record) {
        return Ok(None);
    }
    if let Some((value, error)) = result_arguments(ty)
        && declared_type(value, records).as_deref() == Some(record)
        && let Some(error) = declared_type(error, errors)
    {
        return Ok(Some(error));
    }
    Err(context.reject(ty.span(), "return must be the input record or standard Result of that record and a declared fieldless error"))
}

fn parse_function(
    context: &Context<'_>,
    item: &syn::ItemFn,
    records: &BTreeSet<&str>,
    errors: &BTreeSet<&str>,
) -> Result<Function> {
    public(context, &item.vis, item.sig.ident.span())?;
    context.no_attributes(&item.attrs)?;
    nongeneric(context, &item.sig.generics)?;
    let signature = &item.sig;
    if signature.asyncness.is_some()
        || signature.unsafety.is_some()
        || signature.abi.is_some()
        || signature.variadic.is_some()
    {
        return Err(context.reject(
            signature.span(),
            "functions must be safe synchronous Rust free functions",
        ));
    }
    if !(1..=2).contains(&signature.inputs.len()) {
        return Err(context.reject(
            signature.span(),
            "functions accept one record and an optional i32 parameter",
        ));
    }
    let mut parameters = Vec::new();
    let mut parameter_names = BTreeSet::new();
    for (index, parameter) in signature.inputs.iter().enumerate() {
        let FnArg::Typed(parameter) = parameter else {
            return Err(context.reject(parameter.span(), "receivers are unsupported"));
        };
        context.no_attributes(&parameter.attrs)?;
        let Pat::Ident(pattern) = parameter.pat.as_ref() else {
            return Err(context.reject(
                parameter.pat.span(),
                "parameters must be ordinary named bindings",
            ));
        };
        context.no_attributes(&pattern.attrs)?;
        if pattern.by_ref.is_some() || pattern.subpat.is_some() {
            return Err(context.reject(
                pattern.span(),
                "ref bindings and subpatterns are unsupported",
            ));
        }
        let name = names::identifier(context, &pattern.ident)?;
        if !parameter_names.insert(name.clone()) {
            return Err(context.reject(pattern.ident.span(), "duplicate parameter name"));
        }
        let kind = if index == 0 {
            declared_type(&parameter.ty, records).ok_or_else(|| {
                context.reject(
                    parameter.ty.span(),
                    "first parameter must name a declared record by value",
                )
            })?
        } else {
            if primitive_type(&parameter.ty) != Some("i32") {
                return Err(context.reject(parameter.ty.span(), "second parameter must be i32"));
            }
            if matches!(name.as_str(), "input" | "out_value" | "out_error") {
                return Err(context.reject(
                    pattern.ident.span(),
                    "second parameter conflicts with a reserved physical parameter",
                ));
            }
            "i32".into()
        };
        parameters.push(Parameter {
            name,
            kind,
            span: pattern.ident.span(),
        });
    }
    let record = parameters[0].kind.clone();
    let ReturnType::Type(_, result) = &signature.output else {
        return Err(context.reject(
            signature.span(),
            "function must return its record or Result",
        ));
    };
    let error = result_type(context, result, &record, records, errors)?;
    let mut body = ImplementationCheck {
        context,
        error: None,
    };
    body.visit_block(&item.block);
    if let Some(error) = body.error {
        return Err(error);
    }
    Ok(Function {
        name: names::identifier(context, &signature.ident)?,
        record,
        parameters,
        error,
        span: signature.ident.span(),
    })
}

struct ImplementationCheck<'a, 's> {
    context: &'a Context<'s>,
    error: Option<FormatError>,
}
impl ImplementationCheck<'_, '_> {
    fn reject(&mut self, span: Span, message: &str) {
        if self.error.is_none() {
            self.error = Some(self.context.reject(span, message));
        }
    }
    fn import(&mut self, tree: &syn::UseTree) {
        match tree {
            syn::UseTree::Path(path) => self.import(&path.tree),
            syn::UseTree::Name(name) => {
                if hidden_macro(&name.ident.to_string()) {
                    self.reject(name.ident.span(), "import would expose a macro with hidden compilation inputs or declarations");
                }
            }
            syn::UseTree::Rename(rename) => {
                if hidden_macro(&rename.ident.to_string()) {
                    self.reject(rename.ident.span(), "import alias would expose a macro with hidden compilation inputs or declarations");
                }
            }
            syn::UseTree::Group(group) => {
                for tree in &group.items {
                    self.import(tree);
                }
            }
            syn::UseTree::Glob(_) => {}
        }
    }
    fn tokens(&mut self, tokens: TokenStream) {
        let tokens: Vec<_> = tokens.into_iter().collect();
        for (index, token) in tokens.iter().enumerate() {
            if let TokenTree::Ident(ident) = token {
                if hidden_macro(&ident.to_string())
                    && matches!(tokens.get(index + 1), Some(TokenTree::Punct(p)) if p.as_char() == '!')
                {
                    self.reject(
                        ident.span(),
                        "macro introduces hidden compilation inputs or declarations",
                    );
                }
                // Expression macros can contain blocks with use statements.
                // Parse only an actual tokenized use item; do not inspect the
                // text of literals or pretend macro tokens are all Rust AST.
                if ident == "use"
                    && let Some(end) = tokens[index..].iter().position(
                        |token| matches!(token, TokenTree::Punct(p) if p.as_char() == ';'),
                    )
                {
                    let item = tokens[index..=index + end]
                        .iter()
                        .cloned()
                        .collect::<TokenStream>();
                    if let Ok(item) = syn::parse2::<syn::ItemUse>(item) {
                        self.import(&item.tree);
                    }
                }
            }
            if matches!(token, TokenTree::Punct(p) if p.as_char() == '#') {
                let next = if matches!(tokens.get(index + 1), Some(TokenTree::Punct(p)) if p.as_char() == '!')
                {
                    index + 2
                } else {
                    index + 1
                };
                if matches!(tokens.get(next), Some(TokenTree::Group(g)) if g.delimiter() == proc_macro2::Delimiter::Bracket)
                {
                    self.reject(
                        token.span(),
                        "attributes hidden inside macro input are unsupported",
                    );
                }
            }
            if let TokenTree::Group(group) = token {
                self.tokens(group.stream());
            }
        }
    }
}

fn hidden_macro(name: &str) -> bool {
    matches!(
        name.strip_prefix("r#").unwrap_or(name),
        "include"
            | "include_str"
            | "include_bytes"
            | "env"
            | "option_env"
            | "macro_rules"
            | "global_asm"
            | "asm"
            | "naked_asm"
    )
}
impl<'ast> Visit<'ast> for ImplementationCheck<'_, '_> {
    fn visit_item_use(&mut self, item: &'ast syn::ItemUse) {
        self.import(&item.tree);
        syn::visit::visit_item_use(self, item);
    }

    fn visit_attribute(&mut self, attr: &'ast Attribute) {
        self.reject(
            attr.span(),
            "implementation attributes may not hide compilation inputs or declarations",
        );
    }
    fn visit_item_mod(&mut self, item: &'ast syn::ItemMod) {
        self.reject(
            item.span(),
            "nested modules are outside the single author module recipe",
        );
    }
    fn visit_item_extern_crate(&mut self, item: &'ast syn::ItemExternCrate) {
        self.reject(item.span(), "external crate declarations are unsupported");
    }
    fn visit_item_foreign_mod(&mut self, item: &'ast syn::ItemForeignMod) {
        self.reject(item.span(), "foreign declarations are unsupported");
    }
    fn visit_macro(&mut self, item: &'ast syn::Macro) {
        if item
            .path
            .segments
            .last()
            .is_some_and(|segment| hidden_macro(&segment.ident.to_string()))
        {
            self.reject(
                item.span(),
                "macro introduces hidden compilation inputs or declarations",
            );
        }
        self.tokens(item.tokens.clone());
    }
}

impl Plan {
    fn value(&self, context: &Context<'_>) -> Value {
        json!({
            "namespace": self.namespace,
            "records": self.records.iter().map(|record| json!({"name": record.name, "location": context.location(record.span), "fields": record.fields.iter().map(|field| json!({"name": field.name, "kind": field.kind, "location": context.location(field.span)})).collect::<Vec<_>>()})).collect::<Vec<_>>(),
            "errors": self.errors.iter().map(|error| json!({"name": error.name, "location": context.location(error.span), "variants": error.variants.iter().map(|variant| json!({"name": variant.name, "value": variant.value, "location": context.location(variant.span)})).collect::<Vec<_>>()})).collect::<Vec<_>>(),
            "functions": self.functions.iter().map(|function| json!({"name": function.name, "record": function.record, "parameters": function.parameters.iter().map(|parameter| json!({"name": parameter.name, "kind": parameter.kind})).collect::<Vec<_>>(), "error": function.error, "location": context.location(function.span)})).collect::<Vec<_>>(),
            "witness": witness::render(self),
        })
    }
}
