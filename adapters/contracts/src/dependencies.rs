//! Bundled dependency and cache metadata formats. No filesystem or execution.
use madb_app::{
    Digest, ErrorKind, Id, LogicalPath, QueryError, Result,
    dependencies::{
        CacheManifest, DependencyFormat, DependencyManifest, DependencyRequest, DirectoryDependency,
    },
    input_observation::{DirectoryEntry, DirectoryEntryKind},
    ports::FileReference,
};
use madb_protocol::{SchemaRegistry, canonical_json};
use serde_json::{Value, json};
use std::collections::BTreeMap;

#[derive(Default)]
pub struct JsonDependencyFormat;
impl DependencyFormat for JsonDependencyFormat {
    fn encode_request(&self, request: &DependencyRequest) -> Result<Vec<u8>> {
        request.validate()?;
        encode("dependency-request", request_value(request))
    }
    fn decode_request(&self, bytes: &[u8]) -> Result<DependencyRequest> {
        let value = decode("dependency-request", bytes)?;
        let request = request(&value)?;
        request.validate()?;
        Ok(request)
    }
    fn encode_dependencies(&self, manifest: &DependencyManifest) -> Result<Vec<u8>> {
        manifest.validate()?;
        let mut value = request_value(&DependencyRequest {
            compiler: manifest.compiler.clone(),
            target: manifest.target.clone(),
            sources: manifest.sources.clone(),
            include_directories: manifest.include_directories.clone(),
            absent: manifest.absent.clone(),
            arguments: manifest.arguments.clone(),
            environment: manifest.environment.clone(),
        });
        value["provider"] = json!(manifest.provider.as_str());
        value["context"] = json!(manifest.context.as_str());
        value["discovered"] = json!(
            manifest
                .discovered
                .iter()
                .map(LogicalPath::as_str)
                .collect::<Vec<_>>()
        );
        value["files"] = json!(manifest.files.iter().map(file_value).collect::<Vec<_>>());
        value["directories"]=json!(manifest.directories.iter().map(|directory|json!({"path":directory.path.as_str(),"entries":directory.entries.iter().map(|entry|json!({"name":entry.name.as_str(),"kind":match entry.kind{DirectoryEntryKind::File=>"file",DirectoryEntryKind::Directory=>"directory"}})).collect::<Vec<_>>()})).collect::<Vec<_>>());
        encode("dependency-manifest", value)
    }
    fn decode_dependencies(&self, bytes: &[u8]) -> Result<DependencyManifest> {
        let v = decode("dependency-manifest", bytes)?;
        let r = request(&v)?;
        let manifest = DependencyManifest {
            provider: Id::new(v["provider"].as_str().expect("schema")).map_err(err)?,
            context: digest(&v["context"])?,
            compiler: r.compiler,
            target: r.target,
            sources: r.sources,
            include_directories: r.include_directories,
            absent: r.absent,
            arguments: r.arguments,
            environment: r.environment,
            discovered: paths(&v["discovered"])?,
            files: v["files"]
                .as_array()
                .expect("schema")
                .iter()
                .map(file)
                .collect::<Result<_>>()?,
            directories: v["directories"]
                .as_array()
                .expect("schema")
                .iter()
                .map(|dir| {
                    Ok(DirectoryDependency {
                        path: path(&dir["path"])?,
                        entries: dir["entries"]
                            .as_array()
                            .expect("schema")
                            .iter()
                            .map(|e| {
                                Ok(DirectoryEntry {
                                    name: path(&e["name"])?,
                                    kind: if e["kind"] == "file" {
                                        DirectoryEntryKind::File
                                    } else {
                                        DirectoryEntryKind::Directory
                                    },
                                })
                            })
                            .collect::<Result<_>>()?,
                    })
                })
                .collect::<Result<_>>()?,
        };
        manifest.validate()?;
        Ok(manifest)
    }
    fn encode_cache(&self, manifest: &CacheManifest) -> Result<Vec<u8>> {
        encode(
            "cache-entry",
            json!({"input_key":manifest.input_key.as_str(),"dependency_manifest":file_value(&manifest.dependency_manifest),"files":manifest.files.iter().map(file_value).collect::<Vec<_>>() }),
        )
    }
    fn decode_cache(&self, bytes: &[u8]) -> Result<CacheManifest> {
        let v = decode("cache-entry", bytes)?;
        Ok(CacheManifest {
            input_key: digest(&v["input_key"])?,
            dependency_manifest: file(&v["dependency_manifest"])?,
            files: v["files"]
                .as_array()
                .expect("schema")
                .iter()
                .map(file)
                .collect::<Result<_>>()?,
        })
    }
}
fn request_value(r: &DependencyRequest) -> Value {
    json!({"target":r.target.as_str(),"compiler":file_value(&r.compiler),"sources":r.sources.iter().map(LogicalPath::as_str).collect::<Vec<_>>(),"include_directories":r.include_directories.iter().map(LogicalPath::as_str).collect::<Vec<_>>(),"absent":r.absent.iter().map(LogicalPath::as_str).collect::<Vec<_>>(),"arguments":r.arguments,"environment":r.environment})
}
fn request(v: &Value) -> Result<DependencyRequest> {
    Ok(DependencyRequest {
        compiler: file(&v["compiler"])?,
        target: Id::new(v["target"].as_str().expect("schema")).map_err(err)?,
        sources: paths(&v["sources"])?,
        include_directories: paths(&v["include_directories"])?,
        absent: paths(&v["absent"])?,
        arguments: v["arguments"]
            .as_array()
            .expect("schema")
            .iter()
            .map(|v| v.as_str().expect("schema").to_owned())
            .collect(),
        environment: v["environment"]
            .as_object()
            .expect("schema")
            .iter()
            .map(|(k, v)| (k.clone(), v.as_str().expect("schema").to_owned()))
            .collect::<BTreeMap<_, _>>(),
    })
}
fn paths(v: &Value) -> Result<Vec<LogicalPath>> {
    v.as_array().expect("schema").iter().map(path).collect()
}
fn path(v: &Value) -> Result<LogicalPath> {
    LogicalPath::new(v.as_str().expect("schema")).map_err(err)
}
fn digest(v: &Value) -> Result<Digest> {
    Digest::new(v.as_str().expect("schema")).map_err(err)
}
fn file(v: &Value) -> Result<FileReference> {
    Ok(FileReference {
        path: path(&v["path"])?,
        digest: digest(&v["sha256"])?,
        size: v["size"].as_u64().expect("schema"),
    })
}
fn file_value(file: &FileReference) -> Value {
    json!({"path":file.path.as_str(),"sha256":file.digest.as_str(),"size":file.size})
}
fn encode(name: &str, mut value: Value) -> Result<Vec<u8>> {
    value["schema"] = json!(format!("madb.{name}/1"));
    let bytes = canonical_json(&value).map_err(err)?;
    decode(name, &bytes)?;
    Ok(bytes)
}
fn decode(name: &str, bytes: &[u8]) -> Result<Value> {
    let source = match name {
        "dependency-request" => {
            include_str!("../../../docs/schemas/dependency-request.schema.json")
        }
        "dependency-manifest" => {
            include_str!("../../../docs/schemas/dependency-manifest.schema.json")
        }
        "cache-entry" => include_str!("../../../docs/schemas/cache-entry.schema.json"),
        _ => unreachable!(),
    };
    let schema: Value = serde_json::from_str(source).map_err(err)?;
    SchemaRegistry::validate_bundled(&schema, bytes).map_err(err)
}
fn err(error: impl std::fmt::Display) -> QueryError {
    QueryError::new(
        ErrorKind::InvalidInput,
        "DEP001",
        "dependencies",
        error.to_string(),
    )
}
