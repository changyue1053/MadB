use super::{invalid, wire::*};
use madb_app::Result;
use madb_plan::project::*;
use madb_protocol::Value;
pub(super) fn template(v: &Value) -> Result<CheckedTemplate> {
    let t = Template {
        id: id(&v["id"])?,
        version: version(&v["version"])?,
        madb_compatibility: versions(&v["madb_compatibility"])?,
        default_profile: id(&v["default_profile"])?,
        parameters: array(&v["parameters"])?
            .iter()
            .map(parameter)
            .collect::<Result<_>>()?,
        profiles: array(&v["profiles"])?
            .iter()
            .map(profile)
            .collect::<Result<_>>()?,
        modules: array(&v["modules"])?
            .iter()
            .map(|v| {
                Ok(TemplateModule {
                    id: id(&v["id"])?,
                    role: id(&v["role"])?,
                    directory: optional(&v["directory"], path)?,
                    dependencies: ids(&v["dependencies"])?,
                })
            })
            .collect::<Result<_>>()?,
        boundary_refs: array(&v["boundary_refs"])?
            .iter()
            .map(|v| {
                Ok(BoundaryRef {
                    id: id(&v["id"])?,
                    profile: reference(&v["profile"])?,
                })
            })
            .collect::<Result<_>>()?,
        assets: array(&v["assets"])?
            .iter()
            .map(|v| {
                Ok(TemplateAsset {
                    id: id(&v["id"])?,
                    module: id(&v["module"])?,
                    source: optional(&v["source"], file)?,
                    output: path(&v["output"])?,
                    ownership: ownership(&v["ownership"])?,
                    renderer: optional(&v["renderer"], renderer)?,
                })
            })
            .collect::<Result<_>>()?,
        ide_entries: array(&v["ide_entries"])?
            .iter()
            .map(ide)
            .collect::<Result<_>>()?,
        task_roots: array(&v["task_roots"])?
            .iter()
            .map(|v| {
                Ok(TemplateTaskRoot {
                    id: id(&v["id"])?,
                    module: id(&v["module"])?,
                    recipe: id(&v["recipe"])?,
                    boundary_refs: ids(&v["boundary_refs"])?,
                    targets: ids(&v["targets"])?,
                })
            })
            .collect::<Result<_>>()?,
        validation: array(&v["validation"])?
            .iter()
            .map(|v| {
                Ok(ValidationCheck {
                    id: id(&v["id"])?,
                    level: level(&v["level"])?,
                    target: id(&v["target"])?,
                    task_root: optional(&v["task_root"], id)?,
                    required: boolean(&v["required"])?,
                    minimum_evidence: evidence(&v["minimum_evidence"])?,
                    prerequisites: ids(&v["prerequisites"])?,
                })
            })
            .collect::<Result<_>>()?,
        files: array(&v["files"])?
            .iter()
            .map(file)
            .collect::<Result<_>>()?,
    };
    CheckedTemplate::new(t).map_err(|e| invalid(e.subject, e.message))
}
fn parameter(v: &Value) -> Result<Parameter> {
    let id = id(&v["id"])?;
    let required = boolean(&v["required"])?;
    match string(&v["type"])? {
        "string" => Ok(Parameter::String {
            id,
            required,
            default: optional(&v["default"], |v| Ok(string(v)?.to_owned()))?,
            min_length: uint(&v["min_length"])? as u32,
            max_length: uint(&v["max_length"])? as u32,
            pattern: optional(&v["pattern"], |v| {
                if string(v)? == IDENTIFIER_PATTERN {
                    Ok(StringPattern::Identifier)
                } else {
                    Err(invalid("pattern", "unsupported parameter pattern"))
                }
            })?,
            choices: strings(&v["choices"])?,
        }),
        "boolean" => Ok(Parameter::Boolean {
            id,
            required,
            default: optional(&v["default"], boolean)?,
        }),
        "integer" => Ok(Parameter::Integer {
            id,
            required,
            default: optional(&v["default"], integer)?,
            minimum: integer(&v["minimum"])?,
            maximum: integer(&v["maximum"])?,
        }),
        _ => Err(invalid("parameter", "unknown parameter type")),
    }
}
fn profile(v: &Value) -> Result<TemplateProfile> {
    Ok(TemplateProfile {
        id: id(&v["id"])?,
        targets: array(&v["targets"])?
            .iter()
            .map(|v| {
                Ok(TemplateTarget {
                    id: id(&v["id"])?,
                    family: id(&v["family"])?,
                    boundary_refs: ids(&v["boundary_refs"])?,
                })
            })
            .collect::<Result<_>>()?,
        modules: ids(&v["modules"])?,
        boundary_refs: ids(&v["boundary_refs"])?,
        assets: ids(&v["assets"])?,
        ide_entries: ids(&v["ide_entries"])?,
        task_roots: ids(&v["task_roots"])?,
        validation: ids(&v["validation"])?,
        requirements: Requirements {
            structure: requirements(&v["requirements"]["structure"])?,
            build: requirements(&v["requirements"]["build"])?,
            run: requirements(&v["requirements"]["run"])?,
            open: requirements(&v["requirements"]["open"])?,
        },
    })
}
fn requirements(v: &Value) -> Result<RequirementSet> {
    Ok(RequirementSet {
        hosts: array(&v["hosts"])?
            .iter()
            .map(|v| {
                Ok(HostRequirement {
                    os: id(&v["os"])?,
                    arch: id(&v["arch"])?,
                    versions: versions(v)?,
                })
            })
            .collect::<Result<_>>()?,
        tools: array(&v["tools"])?
            .iter()
            .map(component)
            .collect::<Result<_>>()?,
        ide_entries: ids(&v["ide_entries"])?,
    })
}
fn renderer(v: &Value) -> Result<Renderer> {
    match string(&v["id"])? {
        "madb.render.copy@1" => Ok(Renderer::Copy),
        "madb.render.json-values@1" => Ok(Renderer::JsonValues {
            bindings: array(&v["bindings"])?
                .iter()
                .map(|v| {
                    Ok(JsonBinding {
                        parameter: id(&v["parameter"])?,
                        pointer: string(&v["pointer"])?.into(),
                    })
                })
                .collect::<Result<_>>()?,
        }),
        "madb.render.identifiers@1" => Ok(Renderer::Identifiers {
            language: match string(&v["language"])? {
                "c" => IdentifierLanguage::C,
                "toml" => IdentifierLanguage::Toml,
                "cmake" => IdentifierLanguage::Cmake,
                _ => return Err(invalid("renderer.language", "unknown identifier context")),
            },
            parameters: ids(&v["parameters"])?,
        }),
        _ => Err(invalid("renderer", "unknown renderer")),
    }
}
pub(super) fn ide(v: &Value) -> Result<IdeEntry> {
    Ok(IdeEntry {
        id: id(&v["id"])?,
        module: id(&v["module"])?,
        ide: IdeRequirement {
            id: id(&v["ide"]["id"])?,
            min_inclusive: string(&v["ide"]["min_inclusive"])?.into(),
            max_exclusive: string(&v["ide"]["max_exclusive"])?.into(),
        },
        path: path(&v["path"])?,
        run_target: optional(&v["run_target"], id)?,
        extensions: array(&v["extensions"])?
            .iter()
            .map(|v| {
                Ok(IdeExtension {
                    id: id(&v["id"])?,
                    version: string(&v["version"])?.into(),
                })
            })
            .collect::<Result<_>>()?,
    })
}
