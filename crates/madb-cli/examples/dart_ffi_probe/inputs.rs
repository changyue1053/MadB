use super::Result;
use madb_app::{
    Digest, Id, LogicalPath,
    inputs::{FrozenInputs, capture_inputs},
    ports::{FileReference, ReadWorkspace},
};
use madb_contracts::{
    Document, DocumentKind, SchemaRegistry,
    artifacts::ProvidedFile,
    composition::{
        BoundCompositionInputs, CompositionInputContext, CompositionSchemaPaths,
        RecipeRequirements, bind_composition_inputs,
    },
};
use madb_host::LocalWorkspace;
use std::{
    fs,
    path::{Path, PathBuf},
};

pub const LIMIT: usize = 1024 * 1024;
pub fn id(s: &str) -> Id {
    Id::new(s).expect("fixed identifier")
}
pub fn path(s: &str) -> LogicalPath {
    LogicalPath::new(s).expect("fixed path")
}
pub fn reference(value: &serde_json::Value) -> Result<FileReference> {
    Ok(FileReference {
        path: LogicalPath::new(value["path"].as_str().ok_or("file path")?)?,
        digest: Digest::new(value["sha256"].as_str().ok_or("file digest")?)?,
        size: value["size"].as_u64().ok_or("file size")?,
    })
}
pub fn actual_reference(name: &str, bytes: &[u8]) -> FileReference {
    FileReference {
        path: path(name),
        digest: Digest::of_bytes(bytes),
        size: bytes.len() as u64,
    }
}
pub fn read(workspace: &LocalWorkspace, name: &str, limit: usize) -> Result<Vec<u8>> {
    workspace
        .read(&LogicalPath::new(name)?, limit)?
        .ok_or_else(|| format!("missing input: {name}").into())
}
pub fn directory(root: &Path) -> Result<PathBuf> {
    if !root.is_absolute() || !root.is_dir() || root.canonicalize()? != root {
        return Err("canonical absolute directory required".into());
    }
    Ok(root.to_owned())
}
pub fn capture_tree(root: &Path) -> Result<FrozenInputs> {
    let workspace = LocalWorkspace::open(root)?;
    let mut paths = Vec::new();
    fn visit(root: &Path, dir: &Path, paths: &mut Vec<LogicalPath>) -> Result<()> {
        for item in fs::read_dir(dir)? {
            let item = item?;
            let kind = item.file_type()?;
            if kind.is_dir() {
                visit(root, &item.path(), paths)?;
            } else if kind.is_file() {
                paths.push(LogicalPath::new(
                    item.path()
                        .strip_prefix(root)?
                        .to_str()
                        .ok_or("non UTF8 path")?,
                )?);
            } else {
                return Err("package contains symbolic or special input".into());
            }
            if paths.len() > 4096 {
                return Err("package source count exceeds bound".into());
            }
        }
        Ok(())
    }
    visit(root, root, &mut paths)?;
    paths.sort();
    let references = paths
        .iter()
        .map(|p| read(&workspace, p.as_str(), 64 * LIMIT).map(|b| actual_reference(p.as_str(), &b)))
        .collect::<Result<Vec<_>>>()?;
    Ok(capture_inputs(&workspace, references)?)
}
pub struct Inputs {
    pub root: PathBuf,
    pub workspace: LocalWorkspace,
    pub registry: SchemaRegistry,
    pub project: Document,
    pub project_bytes: Vec<u8>,
    pub lock: Document,
    pub lock_bytes: Vec<u8>,
    pub profile_bytes: Vec<u8>,
    pub frozen: FrozenInputs,
    schemas: CompositionSchemaPaths,
}
impl Inputs {
    pub fn read(&self, name: &str, limit: usize) -> Result<Vec<u8>> {
        read(&self.workspace, name, limit)
    }
    pub fn load(root: &Path) -> Result<Self> {
        if !root.is_absolute() || root.canonicalize()? != root {
            return Err("canonical input root required".into());
        }
        let workspace = LocalWorkspace::open(root)?;
        let registry = SchemaRegistry::new()?;
        let project_bytes = read(&workspace, "project.json", LIMIT)?;
        let lock_bytes = read(&workspace, "lock.json", LIMIT)?;
        let profile_bytes = read(&workspace, "profile.json", LIMIT)?;
        let project = registry.decode(DocumentKind::Project, &project_bytes)?;
        let lock = registry.decode(DocumentKind::Lock, &lock_bytes)?;
        let references = ["inputs", "schemas"]
            .into_iter()
            .flat_map(|k| lock.value()[k].as_array().expect("checked lock"))
            .map(reference)
            .collect::<Result<Vec<_>>>()?;
        let frozen = capture_inputs(&workspace, references)?;
        Ok(Self {
            root: root.to_owned(),
            workspace,
            registry,
            project,
            project_bytes,
            lock,
            lock_bytes,
            profile_bytes,
            frozen,
            schemas: CompositionSchemaPaths {
                common: path("schemas/common.schema.json"),
                project: path("schemas/project.schema.json"),
                lock: path("schemas/lock.schema.json"),
                profile: path("schemas/profile-v2.schema.json"),
                composition: path("schemas/composition.schema.json"),
                extension: path("schemas/extension-manifest.schema.json"),
                contract_ir: path("schemas/contract-ir.schema.json"),
            },
        })
    }
    pub fn context(&self) -> CompositionInputContext<'_> {
        CompositionInputContext {
            project: &self.project,
            lock: &self.lock,
            profile: ProvidedFile {
                path: "profile.json",
                bytes: &self.profile_bytes,
            },
            boundary_id: super::recipe::BOUNDARY,
            inputs: &self.frozen,
            schema_paths: &self.schemas,
        }
    }
    pub fn bind(&self, requirements: &RecipeRequirements) -> Result<BoundCompositionInputs> {
        let bound = bind_composition_inputs(self.context(), requirements)?;
        let packages = bound.composition().value()["packages"]
            .as_array()
            .ok_or("packages")?;
        if packages.len() != 1
            || packages[0]["id"] != super::recipe::PACKAGE
            || packages[0]["version"] != "0.1.0"
        {
            return Err("package outside fixed Dart recipe".into());
        }
        let profile = bound.profile().value();
        if profile["provider"] != serde_json::json!({"language":"c","runtime":"native"})
            || profile["consumer"] != serde_json::json!({"language":"dart","runtime":"dart-native"})
            || profile["abi"] != "darwin-c-ffi"
            || profile["artifact_kind"] != "dart-package"
            || profile["build_system"] != "clang-dart"
            || profile["deployment_target"] != "macOS26.6.2"
        {
            return Err("profile capability outside the fixed tested host".into());
        }
        let features = serde_json::json!([
            "core.exact-numbers@1",
            "core.explicit-close@1",
            "core.extension-composition@1",
            "core.resources@1",
            "core.values@1"
        ]);
        for field in ["required_features", "supported_features"] {
            let mut actual = profile[field].as_array().ok_or("profile features")?.clone();
            actual.sort_by_key(|v| v.as_str().unwrap_or("").to_owned());
            if serde_json::Value::Array(actual) != features {
                return Err("profile features outside the fixed synchronous recipe".into());
            }
        }
        super::recipe::check_assets(self, &bound)?;
        self.verify_external()?;
        Ok(bound)
    }
    pub fn verify(&self) -> Result<()> {
        if self.root.canonicalize()? != self.root {
            return Err("input root moved".into());
        }
        self.frozen.verify(&self.workspace)?;
        for (name, bytes) in [
            ("project.json", &self.project_bytes),
            ("lock.json", &self.lock_bytes),
            ("profile.json", &self.profile_bytes),
        ] {
            if read(&self.workspace, name, LIMIT)? != *bytes {
                return Err("context changed".into());
            }
        }
        self.verify_external()
    }

    // The helper's last check cannot close the host's publication window.
    // Re-read the finite tool/header closure from the already frozen backend
    // and bind each tool tuple to both public profile and lock at every gate.
    fn verify_external(&self) -> Result<()> {
        let bytes = self
            .frozen
            .files()
            .find(|(r, _)| r.path.as_str() == "backend.json")
            .ok_or("backend absent")?
            .1;
        let config: serde_json::Value = serde_json::from_slice(bytes)?;
        let profile: serde_json::Value = serde_json::from_slice(&self.profile_bytes)?;
        let tools = config["tools"].as_object().ok_or("backend tools")?;
        let sdk_root = config["sdk_root"].as_str().ok_or("SDK root")?;
        directory(Path::new(sdk_root))?;
        if config["tools"]["clang"]["arguments"] != serde_json::json!(["-isysroot", sdk_root]) {
            return Err("backend SDK root differs from locked clang arguments".into());
        }
        let base = [
            ("clang", "e04-clang"),
            ("python", "e04-python"),
            ("finalizer", "e04-finalizer"),
            ("libclang", "e04-libclang"),
            ("ffigen", "e04-ffigen"),
        ];
        let extra = [
            ("dart", "e04-dart"),
            ("linker", "e04-linker"),
            ("install_name_tool", "e04-install-name-tool"),
            ("codesign", "e04-codesign"),
            ("otool", "e04-otool"),
        ];
        if tools.len() != base.len() && tools.len() != base.len() + extra.len() {
            return Err("tool closure outside fixed recipe".into());
        }
        for (key, expected_id) in base
            .into_iter()
            .chain(extra.into_iter().filter(|_| tools.len() == 10))
        {
            let t = tools.get(key).ok_or("missing fixed tool")?;
            if key != "clang" && t["arguments"] != serde_json::json!([]) {
                return Err("tool arguments outside fixed recipe".into());
            }
            if key == "python"
                && t["invocation_arguments"]
                    != serde_json::json!(["-I", "-S", "-B", "-X", "pycache_prefix=/dev/null"])
            {
                return Err("Python invocation outside fixed recipe".into());
            }
            let arguments = if key == "python" {
                &t["invocation_arguments"]
            } else {
                &t["arguments"]
            };
            if t["id"] != expected_id
                || t["source"] != "fixed locally observed E04 tool image"
                || !arguments.is_array()
            {
                return Err("backend tool identity differs".into());
            }
            let expected = serde_json::json!({"id":t["id"],"version":t["version"],"sha256":t["sha256"],"source":t["source"],"host":"aarch64-apple-darwin","target":"aarch64-apple-darwin","arguments":arguments});
            for list in [&profile["tools"], &self.lock.value()["tools"]] {
                let actual = list
                    .as_array()
                    .ok_or("locked tools")?
                    .iter()
                    .find(|v| v["id"] == expected_id)
                    .ok_or("tool not declared")?;
                if *actual != expected {
                    return Err("backend tool tuple differs from profile/lock".into());
                }
            }
            external_file(t, 512 * LIMIT)?;
        }
        for field in ["compiler_headers", "build_headers"] {
            if field == "build_headers" && tools.len() == 5 {
                continue;
            }
            let headers = config[field]
                .as_array()
                .ok_or("declared header closure absent")?;
            if headers.is_empty() || headers.len() > 4096 {
                return Err("header closure outside bound".into());
            }
            for header in headers {
                external_file(header, 64 * LIMIT)?;
            }
        }
        Ok(())
    }
}

fn external_file(value: &serde_json::Value, limit: usize) -> Result<()> {
    let path = Path::new(value["path"].as_str().ok_or("external path")?);
    let size = value["size"].as_u64().ok_or("external size")?;
    if !path.is_absolute()
        || path.canonicalize()? != path
        || !fs::symlink_metadata(path)?.file_type().is_file()
        || size > limit as u64
        || fs::metadata(path)?.len() != size
    {
        return Err("external file identity/path differs".into());
    }
    let bytes = fs::read(path)?;
    if bytes.len() as u64 != size || value["sha256"] != Digest::of_bytes(&bytes).as_str() {
        return Err("external tool/header bytes differ".into());
    }
    Ok(())
}

/// Test-only publication fault: the caller must first create this isolated
/// disposable copy. Never accepts the installed/shared tool location.
pub fn corrupt_test_tool(input: &Inputs, run: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let config: serde_json::Value = serde_json::from_slice(&input.read("backend.json", LIMIT)?)?;
    let tool = Path::new(
        config["tools"]["ffigen"]["path"]
            .as_str()
            .ok_or("test tool path")?,
    );
    let expected = run
        .parent()
        .ok_or("run parent")?
        .join("mutable-tools/ffigen");
    if tool != expected || tool.canonicalize()? != expected {
        return Err("fault requires this test's private ffigen copy".into());
    }
    let before = fs::read(tool)?;
    let mut after = before.clone();
    let last = after.last_mut().ok_or("empty test image")?;
    *last ^= 1;
    fs::set_permissions(tool, fs::Permissions::from_mode(0o700))?;
    fs::write(tool, &after)?;
    fs::write(
        run.join("tool-fault.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"path":tool,"before":Digest::of_bytes(&before).as_str(),"after":Digest::of_bytes(&after).as_str(),"scope":"private test copy only"}),
        )?,
    )?;
    Ok(())
}
