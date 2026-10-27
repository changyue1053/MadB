//! Compiled private installation record. It is neither a user-loadable schema
//! nor a source of commands. Installation budgets are separate from projects.
use super::{LockedInitializationFile, LockedInitializationTool, conflict, host, invalid};
use madb_app::{Digest, Id, Result, use_cases::initialize::ToolObservation};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};

const RECORD: &[u8] = include_bytes!("data/installation.json");
const NATIVE: &[u8] = include_bytes!("data/native-observation.json");
const RECORD_LIMIT: usize = 8 * 1024 * 1024;
const INSTALLATION_BYTES: u64 = 2 * 1024 * 1024 * 1024;

struct InstallationRecord {
    tools: BTreeMap<String, LockedInitializationTool>,
    sdk: Vec<LockedInitializationFile>,
    installation: Value,
    python_runtime: Value,
    compiler_runtime: Value,
    sdk_root: PathBuf,
    environment: BTreeMap<String, String>,
}
impl InstallationRecord {
    fn parse(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > RECORD_LIMIT {
            return Err(invalid(
                "installation",
                "compiled installation record exceeds budget",
            ));
        }
        let value: Value =
            serde_json::from_slice(bytes).map_err(|e| invalid("installation", e.to_string()))?;
        let target =
            Id::new(text(&value, "target")?).map_err(|e| invalid("target", e.to_string()))?;
        if target.as_str() != "aarch64-apple-darwin" || text(&value, "system")? != "Darwin" {
            return Err(invalid("installation", "unsupported installed host/target"));
        }
        let native = value["tools"]
            .as_object()
            .ok_or_else(|| invalid("tools", "compiled tools are missing"))?;
        let mut tools = BTreeMap::new();
        for (name, tool) in native.iter().chain(
            [
                ("cmake".into(), value["cmake"].clone()),
                ("ninja".into(), value["ninja"].clone()),
            ]
            .iter()
            .map(|(k, v)| (k, v)),
        ) {
            let file = locked(tool)?;
            let observation = ToolObservation {
                id: Id::new(text(tool, "id")?).map_err(|e| invalid("tool.id", e.to_string()))?,
                version: text(tool, "version")?.into(),
                sha256: file.digest.clone(),
                host: target.clone(),
                target: target.clone(),
                source: format!("system://observed-native-toolchain/{name}"),
                arguments: Vec::new(),
            };
            tools.insert(
                name.to_owned(),
                LockedInitializationTool { file, observation },
            );
        }
        let mut sdk = Vec::new();
        for item in array(&value, "sdk_inputs")? {
            sdk.push(locked(item)?);
        }
        let arguments = array(&value, "clang_arguments")?;
        let sdk_position = arguments
            .iter()
            .position(|v| v.as_str() == Some("-isysroot"))
            .ok_or_else(|| invalid("sdk", "missing fixed sysroot"))?;
        let sdk_root = PathBuf::from(
            arguments
                .get(sdk_position + 1)
                .and_then(Value::as_str)
                .ok_or_else(|| invalid("sdk", "missing fixed sysroot value"))?,
        );
        let mut environment = BTreeMap::new();
        for (key, value) in value["environment"]
            .as_object()
            .ok_or_else(|| invalid("environment", "missing fixed environment"))?
        {
            if !["LC_ALL", "LIBCLANG_PATH"].contains(&key.as_str()) {
                return Err(invalid("environment", "unknown finite recipe environment"));
            }
            environment.insert(
                key.clone(),
                value
                    .as_str()
                    .ok_or_else(|| invalid("environment", "non-string environment"))?
                    .into(),
            );
        }
        environment.insert("PYTHONDONTWRITEBYTECODE".into(), "1".into());
        let result = Self {
            tools,
            sdk,
            installation: value["installation"].clone(),
            python_runtime: value["python_runtime"].clone(),
            compiler_runtime: value["compiler_runtime"].clone(),
            sdk_root,
            environment,
        };
        Ok(result)
    }
    fn verify(&self) -> Result<()> {
        for tool in self.tools.values() {
            tool.verify()?;
        }
        for input in &self.sdk {
            input.verify()?;
        }
        verify_installation(&self.installation)?;
        verify_python(&self.python_runtime)?;
        verify_compiler(&self.compiler_runtime)
    }
}
/// Execution-capable installation: construction still requires every original
/// verification. The doctor observer below never exposes this type.
pub struct InstalledValuesTools {
    record: InstallationRecord,
}
impl InstalledValuesTools {
    pub fn capture() -> Result<Self> {
        let record = InstallationRecord::parse(RECORD)?;
        record.verify()?;
        Ok(Self { record })
    }
    pub fn tool(&self, name: &str) -> Result<&LockedInitializationTool> {
        self.record
            .tools
            .get(name)
            .ok_or_else(|| invalid("tool", format!("finite recipe tool is absent: {name}")))
    }
    pub fn observations(&self) -> Vec<ToolObservation> {
        self.record
            .tools
            .values()
            .map(|tool| tool.observation.clone())
            .collect()
    }
    pub fn native_observation(&self) -> &'static [u8] {
        NATIVE
    }
    pub fn execution_resources(&self) -> &'static [u8] {
        RECORD
    }
    pub fn sdk_root(&self) -> &Path {
        &self.record.sdk_root
    }
    pub fn environment(&self) -> BTreeMap<String, String> {
        self.record.environment.clone()
    }
    pub fn revalidate(&self) -> Result<()> {
        self.record
            .verify()
            .map_err(|e| conflict("installation", e))
    }
}

mod observation;
pub use observation::InstallationObserver;

fn locked(value: &Value) -> Result<LockedInitializationFile> {
    Ok(LockedInitializationFile {
        path: PathBuf::from(text(value, "path")?),
        digest: Digest::new(text(value, "sha256")?)
            .map_err(|e| invalid("installation.sha256", e.to_string()))?,
        size: value["size"]
            .as_u64()
            .ok_or_else(|| invalid("installation.size", "missing integer byte size"))?,
    })
}
fn text<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    value[key]
        .as_str()
        .ok_or_else(|| invalid(key, "compiled installation field is not a string"))
}
fn array<'a>(value: &'a Value, key: &str) -> Result<&'a Vec<Value>> {
    value[key]
        .as_array()
        .ok_or_else(|| invalid(key, "compiled installation field is not an array"))
}
#[cfg(unix)]
fn verify_installation(value: &Value) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let root = PathBuf::from(text(value, "root")?);
    let root_meta = std::fs::symlink_metadata(&root).map_err(|e| host("installation root", e))?;
    if !root_meta.is_dir()
        || root_meta.permissions().mode() & 0o7777
            != value
                .get("mode")
                .or_else(|| value.get("root_mode"))
                .and_then(Value::as_u64)
                .unwrap_or(u64::MAX) as u32
    {
        return Err(invalid("installation root", "root type/mode differs"));
    }
    let entries = array(value, "entries")?;
    if entries.len() > 22_048 {
        return Err(invalid(
            "installation",
            "installation inventory exceeds explicit entry budget",
        ));
    }
    let receipt = value.get("receipt").map(locked).transpose()?;
    if let Some(receipt) = &receipt {
        receipt.verify()?;
    }
    let mut expected = receipt
        .as_ref()
        .map(|r| BTreeSet::from([r.path.clone()]))
        .unwrap_or_default();
    let mut total = receipt.as_ref().map_or(0, |r| r.size);
    let mut files = 0usize;
    let mut links = 0usize;
    for entry in entries {
        let path = PathBuf::from(text(entry, "path")?);
        if !path.is_absolute()
            || path == root
            || !path.starts_with(&root)
            || path.components().any(|p| {
                matches!(
                    p,
                    std::path::Component::ParentDir | std::path::Component::CurDir
                )
            })
            || !expected.insert(path.clone())
        {
            return Err(invalid(
                "installation",
                "duplicate/outside installation inventory path",
            ));
        }
        let metadata =
            std::fs::symlink_metadata(&path).map_err(|e| host("installation member", e))?;
        if metadata.permissions().mode() & 0o7777
            != entry["mode"].as_u64().unwrap_or(u64::MAX) as u32
        {
            return Err(invalid(
                "installation member",
                format!("mode differs: {}", path.display()),
            ));
        }
        match text(entry, "kind")? {
            "file" if metadata.is_file() => {
                let file = locked(entry)?;
                total = total
                    .checked_add(file.size)
                    .ok_or_else(|| invalid("installation", "size overflow"))?;
                files += 1;
                file.verify()?;
            }
            "directory" if metadata.is_dir() => {}
            "symlink" if metadata.is_symlink() => {
                links += 1;
                let target = std::fs::read_link(&path).map_err(|e| host("installation link", e))?;
                if target != Path::new(text(entry, "target")?) {
                    return Err(invalid("installation link", "target differs"));
                }
                let resolved =
                    std::fs::canonicalize(&path).map_err(|e| host("installation link", e))?;
                if !resolved.starts_with(&root) {
                    return Err(invalid("installation link", "target leaves installation"));
                }
            }
            _ => return Err(invalid("installation member", "type differs")),
        }
        if total > INSTALLATION_BYTES || files > 20_000 || links > 2048 {
            return Err(invalid(
                "installation",
                "installation exceeds explicit byte/file/link budget",
            ));
        }
    }
    let mut actual = BTreeSet::new();
    let mut queue = vec![root];
    while let Some(directory) = queue.pop() {
        for item in std::fs::read_dir(directory).map_err(|e| host("installation inventory", e))? {
            let item = item.map_err(|e| host("installation inventory", e))?;
            let path = item.path();
            if value.get("excluded_components").is_some()
                && ["__pycache__", "site-packages"]
                    .contains(&item.file_name().to_string_lossy().as_ref())
            {
                continue;
            }
            if !expected.contains(&path) {
                return Err(invalid(
                    "installation inventory",
                    format!("unexpected member: {}", path.display()),
                ));
            }
            let kind = item
                .file_type()
                .map_err(|e| host("installation inventory", e))?;
            if kind.is_dir() {
                queue.push(path.clone());
            }
            actual.insert(path);
        }
    }
    if actual != expected {
        return Err(invalid(
            "installation inventory",
            "complete installation inventory differs",
        ));
    }
    Ok(())
}
#[cfg(not(unix))]
fn verify_installation(_: &Value) -> Result<()> {
    Err(invalid("platform", "installed recipe requires Unix"))
}

#[cfg(unix)]
fn verify_python(value: &Value) -> Result<()> {
    verify_system_loader(value)?;
    use std::os::unix::fs::PermissionsExt;
    let expected = ["-I", "-S", "-B", "-X", "pycache_prefix=/dev/null"];
    if array(value, "invocation_arguments")?
        .iter()
        .filter_map(Value::as_str)
        .collect::<Vec<_>>()
        != expected
        || array(value, "excluded_components")?
            .iter()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>()
            != ["__pycache__", "site-packages"]
    {
        return Err(invalid(
            "python",
            "runtime exclusions require the fixed isolated interpreter invocation",
        ));
    }
    verify_installation(value)?;
    for path in array(value, "absent_paths")? {
        let path = path
            .as_str()
            .ok_or_else(|| invalid("python", "invalid absent runtime path"))?;
        match std::fs::symlink_metadata(path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(host("python", error)),
            Ok(_) => {
                return Err(invalid(
                    "python",
                    "a previously absent interpreter search path appeared",
                ));
            }
        }
    }
    for link in array(value, "library_links")? {
        let path = Path::new(text(link, "path")?);
        let metadata =
            std::fs::symlink_metadata(path).map_err(|e| host("python library alias", e))?;
        if !metadata.is_symlink()
            || metadata.permissions().mode() & 0o7777
                != link["mode"].as_u64().unwrap_or(u64::MAX) as u32
            || std::fs::read_link(path).map_err(|e| host("python library alias", e))?
                != Path::new(text(link, "target")?)
        {
            return Err(invalid(
                "python library alias",
                "observed alias chain changed",
            ));
        }
    }
    for library in array(value, "libraries")? {
        let mut file = locked(library)?;
        file.path = PathBuf::from(text(library, "image_path")?);
        file.verify()?;
        if std::fs::canonicalize(text(library, "path")?).map_err(|e| host("python library", e))?
            != file.path
        {
            return Err(invalid(
                "python library",
                "install name resolves to another image",
            ));
        }
    }
    Ok(())
}
#[cfg(not(unix))]
fn verify_python(_: &Value) -> Result<()> {
    Err(invalid(
        "platform",
        "installed Python runtime requires Unix",
    ))
}

#[cfg(unix)]
fn verify_system_loader(value: &Value) -> Result<()> {
    if crate::initialization_source::observe_initialization_system_build()?
        != text(value, "system_build")?
    {
        return Err(invalid(
            "runtime.system_build",
            "OS shared-cache build differs from the frozen loader observation",
        ));
    }
    let mut seen = BTreeSet::new();
    for library in array(value, "system_libraries")? {
        let name = library.as_str().ok_or_else(|| {
            invalid(
                "runtime.system_libraries",
                "system loader input must be a name, not a forged file digest",
            )
        })?;
        if !(name.starts_with("/usr/lib/") || name.starts_with("/System/Library/"))
            || Path::new(name).components().any(|part| {
                matches!(
                    part,
                    std::path::Component::ParentDir | std::path::Component::CurDir
                )
            })
            || !seen.insert(name)
        {
            return Err(invalid(
                "runtime.system_libraries",
                "non-system or duplicate shared-cache library observation",
            ));
        }
    }
    Ok(())
}

/// Only the finite installed compiler's observed extra images/aliases. This
/// does not claim a complete compiler tree or arbitrary system sandbox.
#[cfg(unix)]
fn verify_compiler(value: &Value) -> Result<()> {
    verify_system_loader(value)?;
    verify_compiler_files(value)
}
#[cfg(unix)]
fn verify_compiler_files(value: &Value) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let files = array(value, "files")?;
    let links = array(value, "library_links")?;
    if files.is_empty() || files.len() > 20_000 || links.len() > 2048 {
        return Err(invalid(
            "compiler runtime",
            "finite runtime inventory exceeds installation limits",
        ));
    }
    let mut seen = BTreeSet::new();
    let mut total = 0u64;
    for entry in files {
        let file = locked(entry)?;
        if !seen.insert(file.path.clone()) {
            return Err(invalid("compiler runtime", "duplicate runtime image"));
        }
        total = total
            .checked_add(file.size)
            .ok_or_else(|| invalid("compiler runtime", "runtime size overflow"))?;
        if total > INSTALLATION_BYTES {
            return Err(invalid(
                "compiler runtime",
                "runtime image bytes exceed installation budget",
            ));
        }
        file.verify()?;
    }
    for link in links {
        let path = PathBuf::from(text(link, "path")?);
        if !seen.insert(path.clone()) {
            return Err(invalid("compiler runtime", "duplicate runtime alias"));
        }
        let metadata =
            std::fs::symlink_metadata(&path).map_err(|e| host("compiler runtime alias", e))?;
        if !metadata.is_symlink()
            || metadata.permissions().mode() & 0o7777
                != link["mode"].as_u64().unwrap_or(u64::MAX) as u32
            || std::fs::read_link(&path).map_err(|e| host("compiler runtime alias", e))?
                != Path::new(text(link, "target")?)
        {
            return Err(invalid(
                "compiler runtime alias",
                "recorded compiler alias changed",
            ));
        }
    }
    Ok(())
}
#[cfg(not(unix))]
fn verify_compiler(_: &Value) -> Result<()> {
    Err(invalid(
        "platform",
        "installed compiler runtime requires Unix",
    ))
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::{fs, os::unix::fs::PermissionsExt};
    fn record(root: &Path) -> Value {
        let file = root.join("fixed");
        let bytes = fs::read(&file).unwrap();
        serde_json::json!({"root":root,"mode":fs::metadata(root).unwrap().permissions().mode()&0o7777,"entries":[{"path":file,"kind":"file","mode":fs::metadata(&file).unwrap().permissions().mode()&0o7777,"sha256":Digest::of_bytes(&bytes).as_str(),"size":bytes.len()}]})
    }
    #[test]
    fn complete_resource_inventory_rejects_extra_missing_and_changed_members() {
        let root = crate::locations::temporary_parent()
            .unwrap()
            .join(crate::locations::unique_run_name().as_str());
        fs::create_dir(&root).unwrap();
        fs::write(root.join("fixed"), b"resource").unwrap();
        let value = record(&root);
        verify_installation(&value).unwrap();
        fs::write(root.join("extra"), b"extra").unwrap();
        assert_eq!(
            verify_installation(&value).unwrap_err().kind,
            madb_app::ErrorKind::InvalidInput
        );
        fs::remove_file(root.join("extra")).unwrap();
        fs::write(root.join("fixed"), b"different resource").unwrap();
        assert_eq!(
            verify_installation(&value).unwrap_err().kind,
            madb_app::ErrorKind::InvalidInput
        );
        fs::remove_file(root.join("fixed")).unwrap();
        assert_eq!(
            verify_installation(&value).unwrap_err().kind,
            madb_app::ErrorKind::Host
        );
        fs::remove_dir(&root).unwrap();
    }
    #[test]
    fn resource_permissions_and_fixed_symlink_targets_are_identity() {
        let root = crate::locations::temporary_parent()
            .unwrap()
            .join(crate::locations::unique_run_name().as_str());
        fs::create_dir(&root).unwrap();
        fs::write(root.join("fixed"), b"resource").unwrap();
        let mut value = record(&root);
        let original = fs::metadata(root.join("fixed")).unwrap().permissions();
        fs::set_permissions(root.join("fixed"), fs::Permissions::from_mode(0o700)).unwrap();
        assert!(verify_installation(&value).is_err());
        fs::set_permissions(root.join("fixed"), original).unwrap();
        std::os::unix::fs::symlink("fixed", root.join("alias")).unwrap();
        value["entries"].as_array_mut().unwrap().push(serde_json::json!({"path":root.join("alias"),"kind":"symlink","mode":fs::symlink_metadata(root.join("alias")).unwrap().permissions().mode()&0o7777,"target":"fixed"}));
        verify_installation(&value).unwrap();
        fs::remove_file(root.join("alias")).unwrap();
        std::os::unix::fs::symlink("missing", root.join("alias")).unwrap();
        assert!(verify_installation(&value).is_err());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn finite_compiler_images_and_aliases_reject_missing_changed_and_retargeted_bytes() {
        let root = crate::locations::temporary_parent()
            .unwrap()
            .join(crate::locations::unique_run_name().as_str());
        fs::create_dir(&root).unwrap();
        let image = root.join("driver.dylib");
        fs::write(&image, b"fixed image").unwrap();
        std::os::unix::fs::symlink("driver.dylib", root.join("driver-alias")).unwrap();
        let record = serde_json::json!({"files":[{"path":image,"sha256":Digest::of_bytes(b"fixed image").as_str(),"size":11}],"library_links":[{"path":root.join("driver-alias"),"target":"driver.dylib","mode":fs::symlink_metadata(root.join("driver-alias")).unwrap().permissions().mode()&0o7777}]});
        verify_compiler_files(&record).unwrap();
        fs::write(&image, b"changed image").unwrap();
        assert_eq!(
            verify_compiler_files(&record).unwrap_err().kind,
            madb_app::ErrorKind::InvalidInput
        );
        fs::remove_file(&image).unwrap();
        assert_eq!(
            verify_compiler_files(&record).unwrap_err().kind,
            madb_app::ErrorKind::Host
        );
        fs::write(&image, b"fixed image").unwrap();
        fs::remove_file(root.join("driver-alias")).unwrap();
        std::os::unix::fs::symlink("other.dylib", root.join("driver-alias")).unwrap();
        assert_eq!(
            verify_compiler_files(&record).unwrap_err().kind,
            madb_app::ErrorKind::InvalidInput
        );
        fs::remove_dir_all(root).unwrap();
    }
    #[cfg(target_os = "macos")]
    #[test]
    fn shared_cache_names_bind_the_actual_build_without_fake_file_hashes() {
        let build = crate::initialization_source::observe_initialization_system_build().unwrap();
        let valid = serde_json::json!({"system_build":build,"system_libraries":["/usr/lib/libSystem.B.dylib"]});
        verify_system_loader(&valid).unwrap();
        let mut wrong = valid.clone();
        wrong["system_build"] = serde_json::json!("different-build");
        assert!(verify_system_loader(&wrong).is_err());
        for name in [
            serde_json::json!("/opt/foreign/lib.dylib"),
            serde_json::json!("/usr/lib/../../opt/foreign.dylib"),
            serde_json::json!({"path":"/usr/lib/libSystem.B.dylib","sha256":"fabricated"}),
        ] {
            let mut wrong = valid.clone();
            wrong["system_libraries"] = serde_json::json!([name]);
            assert!(verify_system_loader(&wrong).is_err());
        }
    }
}
