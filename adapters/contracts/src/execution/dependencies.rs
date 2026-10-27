//! Check dependency preparation against the selected target and locked tool image.
use super::{digest, files::check_bytes, id, invalid, spec::ExecutionTool};
use crate::query::{PreparedQuery, SnapshotAdapter};
use madb_app::{
    Id, Result,
    ports::{ConfigurationBytes, FileReference, SnapshotFormat},
    process::MAX_TOOL_BYTES,
};

pub(super) fn bind_tool(
    prepared: &PreparedQuery,
    component: &ExecutionTool,
    installed: &FileReference,
) -> Result<()> {
    bind_tool_values(&prepared.profile, &prepared.lock, component, installed)
}

pub(crate) fn bind_tool_values(
    profile: &crate::Document,
    lock: &crate::Document,
    component: &ExecutionTool,
    installed: &FileReference,
) -> Result<()> {
    let tool = profile.value()["tools"]
        .as_array()
        .expect("validated")
        .iter()
        .find(|tool| {
            tool["id"].as_str() == Some(component.id.as_str())
                && tool["version"].as_str() == Some(&component.version)
                && tool["sha256"].as_str() == Some(component.digest.as_str())
        })
        .ok_or_else(|| {
            invalid(
                "LOCK004",
                "tool",
                "recipe tool is not an exact selected profile image",
            )
        })?;
    if !lock.value()["tools"]
        .as_array()
        .expect("validated")
        .contains(tool)
        || installed.size == 0
        || installed.size > MAX_TOOL_BYTES
        || installed.digest != component.digest
    {
        return Err(invalid(
            "LOCK004",
            "installed_tool",
            "selected image is not exactly locked or installed identity is invalid",
        ));
    }
    Ok(())
}

/// Dependency discovery may execute only one unambiguous image from the exact
/// selected profile and complete lock entry, even before metadata is available.
pub fn validate_dependency_tool(
    configuration: ConfigurationBytes<'_>,
    compiler: &FileReference,
) -> Result<()> {
    let adapter = SnapshotAdapter::new()?;
    let references = adapter.request_files(configuration.request)?;
    check_bytes(&references.lock, configuration.lock)?;
    check_bytes(&references.profile, configuration.profile)?;
    let (prepared, _) = adapter.prepare(configuration)?;
    let matching = prepared.profile.value()["tools"]
        .as_array()
        .expect("validated")
        .iter()
        .filter(|tool| tool["sha256"].as_str() == Some(compiler.digest.as_str()))
        .collect::<Vec<_>>();
    if matching.len() != 1 {
        return Err(invalid(
            "LOCK004",
            "compiler",
            "dependency compiler must match one selected profile image",
        ));
    }
    let tool = matching[0];
    bind_tool(
        &prepared,
        &ExecutionTool {
            id: id(&tool["id"])?,
            version: tool["version"].as_str().expect("validated").to_owned(),
            digest: digest(&tool["sha256"])?,
        },
        compiler,
    )
}

/// The native preparation profile is explicit and narrower than general target
/// support. Do not execute a host-default compiler for a different selected ABI.
pub fn validate_dependency_target(
    configuration: ConfigurationBytes<'_>,
    target: &Id,
) -> Result<()> {
    let adapter = SnapshotAdapter::new()?;
    let (prepared, _) = adapter.prepare(configuration)?;
    let selected = prepared.profile.value()["target"]
        .as_str()
        .expect("validated target");
    let matches = match target.as_str() {
        "arm64-apple-macos11.0" => {
            matches!(selected, "aarch64-apple-darwin" | "arm64-apple-macos11.0")
        }
        "x86_64-apple-macos10.15" => {
            matches!(selected, "x86_64-apple-darwin" | "x86_64-apple-macos10.15")
        }
        "aarch64-unknown-linux-gnu" | "x86_64-unknown-linux-gnu" => selected == target.as_str(),
        _ => false,
    };
    if matches {
        Ok(())
    } else {
        Err(invalid(
            "DEP003",
            "target",
            "native preparation target differs from the selected profile or is unsupported",
        ))
    }
}
