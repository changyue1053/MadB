//! Validate real C03 probe records using C02's public format and digest APIs.
//! This example accepts a trusted, locally generated probe directory. It is not
//! the production host filesystem sandbox, reader or frozen build executor.
use madb_contracts::{
    Document, DocumentKind, SchemaRegistry,
    artifacts::{ProvidedFile, verify_reference},
    capability, profile_digest, project_digest,
};
use serde_json::Value;
use std::{error::Error, fs, path::Path};

fn load(registry: &SchemaRegistry, root: &Path, name: &str, kind: DocumentKind) -> Document {
    registry
        .decode(kind, &fs::read(root.join(name)).expect("probe file"))
        .expect("probe schema")
}

fn verify(root: &Path, reference: &Value) -> Result<(), Box<dyn Error>> {
    let path = reference["path"].as_str().ok_or("missing file path")?;
    let resolved = root.join(path).canonicalize()?;
    if !resolved.starts_with(root) {
        return Err("probe reference escaped its root".into());
    }
    let bytes = fs::read(resolved)?;
    verify_reference(
        reference,
        &ProvidedFile {
            path,
            bytes: &bytes,
        },
    )?;
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let root = std::env::args()
        .nth(1)
        .ok_or("supply the C03 output directory")?;
    let root = Path::new(&root).canonicalize()?;
    let registry = SchemaRegistry::new()?;
    let project = load(&registry, &root, "project.json", DocumentKind::Project);
    let lock = load(&registry, &root, "madb.lock.json", DocumentKind::Lock);
    if lock.value()["project_digest"] != project_digest(&project)? {
        return Err("project semantic digest mismatch".into());
    }
    for group in ["inputs", "schemas"] {
        for reference in lock.value()[group].as_array().ok_or("missing inventory")? {
            verify(&root, reference)?;
        }
    }
    let locked_profiles = lock.value()["profiles"]
        .as_array()
        .ok_or("missing profiles")?;
    let boundaries = project.value()["boundaries"]
        .as_array()
        .ok_or("missing boundaries")?;
    let locked_tools = lock.value()["tools"].as_array().ok_or("missing tools")?;
    for item in locked_profiles {
        verify(&root, &item["file"])?;
        let name = item["file"]["path"]
            .as_str()
            .ok_or("missing profile path")?;
        let profile = load(&registry, &root, name, DocumentKind::Profile);
        capability(&profile)?;
        if item["semantic_digest"] != profile_digest(&profile)? {
            return Err("profile semantic digest mismatch".into());
        }
        verify(&root, &profile.value()["sdk"])?;
        if !boundaries
            .iter()
            .any(|b| b["profile"] == item["file"] && b["target"] == profile.value()["target"])
        {
            return Err("profile is not selected by the project".into());
        }
        for tool in profile.value()["tools"]
            .as_array()
            .ok_or("missing profile tools")?
        {
            if !locked_tools.contains(tool) || tool["target"] != profile.value()["target"] {
                return Err("profile tool is not exactly locked".into());
            }
        }
    }
    let observations: Value = serde_json::from_slice(&fs::read(root.join("toolchain-lock.json"))?)?;
    for output in observations["outputs"]
        .as_array()
        .ok_or("missing probe outputs")?
    {
        verify(&root, output)?;
    }
    println!("C03_LOCK_OK profiles={}", locked_profiles.len());
    Ok(())
}
