//! Assemble the existing pure format, inventory and renderer adapters. This
//! module cannot obtain packages or touch a destination directory.
use super::catalog::TrustedTemplateRecord;
use madb_app::inputs::capture_inputs;
use madb_app::ports::{FileReference, ReadWorkspace};
use madb_app::use_cases::initialize::{FrozenInitRequest, InitialFile, PreparedProject};
use madb_app::{ErrorKind, LogicalPath, QueryError, Result};
use madb_contracts::SchemaRegistry;
use madb_contracts::project_init::{CatalogTemplateIdentity, ProjectInitAdapter};
use madb_plan::project::{CheckedTemplate, FileOwnership};
use madb_templates::{AssetError, AssetErrorKind, CheckedInventory, parse_archive, render_asset};
use std::collections::BTreeMap;

pub(crate) fn load(
    record: &TrustedTemplateRecord,
    adapter: &ProjectInitAdapter,
) -> Result<(CheckedTemplate, CheckedInventory)> {
    let archive = parse_archive(
        record.bytes().to_vec(),
        &record.archive.sha256,
        record.archive.size,
    )
    .map_err(asset_error)?;
    let manifest = archive
        .entry_bytes(&record.manifest.file.path)
        .ok_or_else(|| {
            invalid(
                "manifest",
                "bundled archive does not contain its fixed manifest",
            )
        })?;
    if madb_app::Digest::of_bytes(manifest) != record.manifest.file.sha256
        || manifest.len() as u64 != record.manifest.file.size
    {
        return Err(invalid(
            "manifest",
            "original manifest differs from its installed pin",
        ));
    }
    let template = adapter.decode_template(manifest)?;
    if template.template().id != record.id || template.template().version != record.version {
        return Err(invalid(
            "manifest",
            "manifest identity differs from its installed catalog record",
        ));
    }
    let inventory = archive
        .check_inventory(&record.manifest.file, &template.template().files)
        .map_err(asset_error)?;
    Ok((template, inventory))
}

pub(crate) fn project(
    record: &TrustedTemplateRecord,
    package: &CheckedInventory,
    request: &FrozenInitRequest,
    adapter: &ProjectInitAdapter,
) -> Result<PreparedProject> {
    project_inner(record, package, request, adapter, None)
}
pub(crate) fn project_with_tool(
    record: &TrustedTemplateRecord,
    package: &CheckedInventory,
    request: &FrozenInitRequest,
    adapter: &ProjectInitAdapter,
    tool: madb_app::use_cases::project_build::ToolIdentity,
) -> Result<PreparedProject> {
    project_inner(record, package, request, adapter, Some(tool))
}
fn project_inner(
    record: &TrustedTemplateRecord,
    package: &CheckedInventory,
    request: &FrozenInitRequest,
    adapter: &ProjectInitAdapter,
    tool: Option<madb_app::use_cases::project_build::ToolIdentity>,
) -> Result<PreparedProject> {
    let selected = request.selected();
    for task in selected
        .template()
        .task_roots
        .iter()
        .filter(|t| selected.profile().task_roots.contains(&t.id))
    {
        record.recipe(&task.recipe)?;
    }
    let references = package
        .archive()
        .entries()
        .iter()
        .map(|entry| {
            let file = entry.identity();
            FileReference {
                path: file.path.clone(),
                digest: file.sha256.clone(),
                size: file.size,
            }
        })
        .collect();
    let captured = capture_inputs(&ArchiveReader(package), references)?;
    adapter.check_template_inputs(
        selected,
        &record.manifest.file,
        &record.archive,
        record.bytes(),
        &captured,
        CatalogTemplateIdentity {
            id: &record.id,
            version: record.version,
        },
    )?;
    let registry = SchemaRegistry::new().map_err(|e| invalid("schema registry", e.to_string()))?;
    let mut rendered = BTreeMap::new();
    for asset in selected
        .template()
        .assets
        .iter()
        .filter(|a| selected.profile().assets.contains(&a.id))
    {
        let bytes = render_asset(asset, package, &request.projection().parameter_values, &registry, |_| {
            Err(AssetError { kind:AssetErrorKind::DocumentValidation, subject:asset.id.to_string(),
                message:"this finite installed recipe has no generated MadB JSON asset; a new semantic validator is required".into() })
        }).map_err(asset_error)?;
        if let Some(bytes) = bytes {
            rendered.insert(asset.output.clone(), bytes);
        }
    }
    if selected.template().version
        == madb_plan::project::Version::parse("0.2.0").expect("fixed version")
    {
        use madb_app::use_cases::project_build::{
            Descriptor, ProjectBuildFormat, TemplateIdentity,
        };
        let tool = tool.ok_or_else(|| {
            invalid(
                "tool",
                "the project template needs its actual source-bound image identity",
            )
        })?;
        let namespace = match request
            .projection()
            .parameter_values
            .get(&madb_app::Id::new("namespace").expect("fixed parameter"))
        {
            Some(madb_plan::project::ParameterValue::String(value)) => value.clone(),
            _ => return Err(invalid("namespace", "resolved namespace is absent")),
        };
        let reference = |name: &str, schema: &str| -> Result<madb_plan::project::FileReference> {
            let mut file = selected
                .template()
                .files
                .iter()
                .find(|f| f.path.as_str() == name)
                .cloned()
                .ok_or_else(|| invalid(name, "original input is absent"))?;
            file.path = LogicalPath::new(format!(".madb/template-input/{name}"))
                .map_err(|e| invalid(name, e.to_string()))?;
            Ok(madb_plan::project::FileReference {
                file,
                schema: madb_app::Id::new(schema).expect("fixed schema"),
            })
        };
        let mut archive = record.archive.clone();
        archive.path = LogicalPath::new(".madb/template-package.tar").expect("fixed path");
        let descriptor = Descriptor {
            template: TemplateIdentity {
                id: record.id.clone(),
                version: record.version.to_string(),
                archive,
            },
            namespace,
            profile: reference("profile.json", "madb.profile/1")?,
            sdk: reference("toolchain.json", "madb.native-toolchain/1")?,
            tool,
        };
        let format = madb_contracts::project_build::ProjectBuildAdapter::new()?;
        let bytes = format.encode_descriptor(&descriptor)?;
        let origin = madb_host::project_build::origin_bytes(&descriptor, &bytes)?;
        rendered.insert(
            LogicalPath::new(".madb/project-build.json").expect("fixed path"),
            bytes,
        );
        rendered.insert(
            LogicalPath::new(".madb/project-origin.json").expect("fixed path"),
            origin,
        );
    }
    let plan = selected
        .plan_with_provenance(&record.manifest.file)
        .map_err(|e| invalid(&e.subject, e.message))?;
    let mut files = Vec::new();
    for entry in plan.files() {
        if entry.ownership == FileOwnership::LocalState {
            files.push(InitialFile::local_state(
                entry.module.clone(),
                entry.path.clone(),
            ));
            continue;
        }
        let bytes = if entry.path.as_str() == ".madb/template-package.tar" {
            record.bytes().to_vec()
        } else if let Some(source) = entry.path.as_str().strip_prefix(".madb/template-input/") {
            let path = LogicalPath::new(source).map_err(|e| invalid(source, e.to_string()))?;
            package
                .archive()
                .entry_bytes(&path)
                .ok_or_else(|| invalid(source, "missing original provenance input"))?
                .to_vec()
        } else {
            rendered
                .remove(&entry.path)
                .ok_or_else(|| invalid(entry.path.as_str(), "initial plan has no rendered bytes"))?
        };
        files.push(InitialFile::rendered(
            entry.module.clone(),
            entry.path.clone(),
            entry.ownership,
            bytes,
        )?);
    }
    if !rendered.is_empty() {
        return Err(invalid(
            "rendered assets",
            "rendered files exceed the complete initial plan",
        ));
    }
    PreparedProject::new(
        request,
        record.manifest.file.clone(),
        record.archive.clone(),
        files,
    )
}
struct ArchiveReader<'a>(&'a CheckedInventory);
impl ReadWorkspace for ArchiveReader<'_> {
    fn read(&self, path: &LogicalPath, limit: usize) -> Result<Option<Vec<u8>>> {
        self.0
            .archive()
            .entry_bytes(path)
            .map(|bytes| {
                if bytes.len() > limit {
                    return Err(invalid(
                        path.as_str(),
                        "frozen package input exceeds capture budget",
                    ));
                }
                Ok(bytes.to_vec())
            })
            .transpose()
    }
}
fn asset_error(error: AssetError) -> QueryError {
    invalid(&error.subject, error.message)
}
fn invalid(subject: &str, message: impl Into<String>) -> QueryError {
    QueryError::new(ErrorKind::InvalidInput, "PLAN001", subject, message)
}
