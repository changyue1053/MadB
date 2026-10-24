//! Test-only composition for real restricted Clang discovery and cache IO.
//! Fixtures supply tool identities; no fixture can assert a complete closure.
use madb_app::{
    Digest, ErrorKind, LogicalPath, QueryError, Result,
    dependencies::DependencyFormat,
    inputs::FrozenInputs,
    process::{ExecutionControl, ProcessExecution, ProcessResult},
};
use madb_contracts::dependencies::JsonDependencyFormat;
use madb_host::{
    LocalWorkspace,
    cache::LocalDependencyCache,
    dependencies::{discover_clang, preprocess_clang},
};
use serde_json::{Value, json};
use std::{
    fs,
    path::Path,
    time::{Duration, Instant},
};

fn text<'a>(value: &'a Value, key: &str) -> &'a str {
    value[key].as_str().expect("fixture string")
}
fn failure(error: &QueryError) -> Value {
    json!({"status":"error","codes":error.findings.iter().map(|f|f.code).collect::<Vec<_>>(),
        "message":error.to_string(),"owners_drained":true})
}
fn fixture_error(error: impl std::fmt::Display) -> QueryError {
    QueryError::new(ErrorKind::Host, "TEST_DRIVER", "fixture", error.to_string())
}
fn process_details(execution: Option<&ProcessResult<ProcessExecution>>) -> Value {
    match execution {
        None => json!({"started":false,"owners_drained":true}),
        Some(Err(error)) => {
            json!({"started":false,"owners_drained":true,"diagnostic":error.to_string()})
        }
        Some(Ok(execution)) => match &execution.process {
            None => json!({"started":false,"owners_drained":true}),
            Some(run) => json!({"started":true,
                "owners_drained":run.pending_cleanup.is_none() && run.report.cleanup.locally_complete(),
                "termination":format!("{:?}",run.report.termination),
                "stderr":String::from_utf8_lossy(&run.report.stderr.bytes),
                "diagnostic":run.report.diagnostic.as_ref().map(ToString::to_string)}),
        },
    }
}
fn drain(mut poll: impl FnMut() -> ProcessResult<bool>) -> Result<()> {
    let start = Instant::now();
    while !poll().map_err(fixture_error)? {
        if start.elapsed() >= Duration::from_secs(5) {
            return Err(fixture_error("owned process did not finish cleanup"));
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    Ok(())
}
fn output_value(files: &FrozenInputs) -> Value {
    Value::Object(
        files
            .files()
            .map(|(file, bytes)| (file.path.as_str().to_owned(), json!(bytes)))
            .collect(),
    )
}
fn run(config: &Value) -> Result<Value> {
    let format = JsonDependencyFormat;
    let request =
        format.decode_request(&serde_json::to_vec(&config["request"]).map_err(fixture_error)?)?;
    let source_root = Path::new(text(config, "source_root"));
    let tool_root = Path::new(text(config, "tool_root"));
    let run_parent = Path::new(text(config, "run_parent"));
    let name = text(config, "run_name");
    let discovery_name = LogicalPath::new(format!("{name}-discovery")).map_err(fixture_error)?;
    let mut discovery = discover_clang(
        source_root,
        tool_root,
        run_parent,
        &discovery_name,
        &request,
        Digest::new(text(config, "context")).map_err(fixture_error)?,
        &format,
        &ExecutionControl::default(),
    )?;
    if let Err(error) = drain(|| discovery.poll_cleanup()) {
        let details = process_details(discovery.execution());
        return Ok(json!({"status":"error","message":error.to_string(),
            "owners_drained":details["owners_drained"],"discovery":details}));
    }
    let discovery_details = process_details(discovery.execution());
    let owners_drained = discovery_details["owners_drained"] == true;
    let Some(snapshot) = discovery.snapshot.as_ref() else {
        return Ok(
            json!({"status":"uncacheable","owners_drained":owners_drained,
            "reason":discovery.reason.as_ref().map(|r|r.message.as_str()),"discovery":discovery_details}),
        );
    };
    let source = LocalWorkspace::open(source_root)?;
    let cache = LocalDependencyCache::open(Path::new(text(config, "cache_root")), false)?;
    let output = LogicalPath::new("generated.c").expect("constant");
    let cached = cache.load(&source, snapshot, std::slice::from_ref(&output), &format)?;
    let (files, cache_hit, preprocess_details) = if let Some(files) = cached {
        (files, true, json!({"started":false,"owners_drained":true}))
    } else {
        let generate_name =
            LogicalPath::new(format!("{name}-preprocess")).map_err(fixture_error)?;
        let mut generated = preprocess_clang(
            source_root,
            tool_root,
            run_parent,
            &generate_name,
            &request,
            snapshot,
            &output,
            &ExecutionControl::default(),
        )?;
        if let Err(error) = drain(|| generated.poll_cleanup()) {
            let details = process_details(Some(generated.execution()));
            return Ok(json!({"status":"error","message":error.to_string(),
                "owners_drained":owners_drained && details["owners_drained"] == true,"preprocess":details}));
        }
        let details = process_details(Some(generated.execution()));
        let Some(files) = generated.value().cloned() else {
            return Ok(
                json!({"status":"error","owners_drained":owners_drained && details["owners_drained"] == true,
                "discovery":discovery_details,"preprocess":details}),
            );
        };
        snapshot.verify(&source)?;
        cache.store(snapshot, &files, &format)?;
        (files, false, details)
    };
    let entry = cache.entry_path(snapshot);
    Ok(
        json!({"status":"ok","owners_drained":owners_drained && preprocess_details["owners_drained"] == true,
        "cache_hit":cache_hit,"input_key":snapshot.key().as_str(),"files":output_value(&files),
        "manifest":serde_json::from_slice::<Value>(snapshot.manifest_bytes()).map_err(fixture_error)?,
        "discovery":discovery_details,"preprocess":preprocess_details,
        "cache_paths":{"entry":entry,"output":entry.join("inputs/generated.c"),
            "dependency":entry.join("work/dependency.json"),"metadata":entry.join("work/cache.json")}}),
    )
}
fn main() {
    let path = std::env::args().nth(1).expect("fixture config path");
    let config: Value =
        serde_json::from_slice(&fs::read(path).expect("fixture config")).expect("fixture JSON");
    let result = match run(&config) {
        Ok(value) => value,
        Err(error) => failure(&error),
    };
    println!("{result}");
}

#[cfg(test)]
mod format_tests {
    use super::*;
    use madb_app::{
        Id,
        dependencies::{CacheManifest, DependencyManifest, DependencyRequest, DirectoryDependency},
        input_observation::{DirectoryEntry, DirectoryEntryKind},
        ports::FileReference,
    };
    use std::collections::BTreeMap;

    fn reference(path: &str, bytes: &[u8]) -> FileReference {
        FileReference {
            path: LogicalPath::new(path).unwrap(),
            digest: Digest::of_bytes(bytes),
            size: bytes.len() as u64,
        }
    }
    fn request() -> DependencyRequest {
        DependencyRequest {
            compiler: reference("clang", b"fixture tool identity"),
            target: Id::new("x86_64-unknown-linux-gnu").unwrap(),
            sources: vec![LogicalPath::new("source/main.c").unwrap()],
            include_directories: vec![],
            absent: vec![],
            arguments: vec!["-std=c11".into()],
            environment: BTreeMap::new(),
        }
    }
    #[test]
    fn dependency_request_roundtrips_and_rejects_forged_completeness() {
        let format = JsonDependencyFormat;
        let expected = request();
        let encoded = format.encode_request(&expected).unwrap();
        assert_eq!(format.decode_request(&encoded).unwrap(), expected);
        let mut malformed: Value = serde_json::from_slice(&encoded).unwrap();
        malformed["complete"] = json!(true);
        assert!(
            format
                .decode_request(&serde_json::to_vec(&malformed).unwrap())
                .is_err()
        );
        malformed.as_object_mut().unwrap().remove("complete");
        malformed["compiler"]["size"] = json!(-1);
        assert!(
            format
                .decode_request(&serde_json::to_vec(&malformed).unwrap())
                .is_err()
        );
    }
    #[test]
    fn discovered_manifest_and_cache_metadata_roundtrip_and_reject_unknown_fields() {
        let format = JsonDependencyFormat;
        let request = request();
        let expected = DependencyManifest {
            provider: Id::new("clang-freestanding/1").unwrap(),
            context: Digest::of_bytes(b"fixture context"),
            compiler: request.compiler,
            target: request.target,
            sources: request.sources.clone(),
            include_directories: request.include_directories,
            arguments: request.arguments,
            environment: request.environment,
            files: vec![reference("source/main.c", b"int value;\n")],
            discovered: request.sources,
            directories: vec![DirectoryDependency {
                path: LogicalPath::new("source").unwrap(),
                entries: vec![DirectoryEntry {
                    name: LogicalPath::new("main.c").unwrap(),
                    kind: DirectoryEntryKind::File,
                }],
            }],
            absent: request.absent,
        };
        let bytes = format.encode_dependencies(&expected).unwrap();
        assert_eq!(format.decode_dependencies(&bytes).unwrap(), expected);
        let cache = CacheManifest {
            input_key: Digest::of_bytes(&bytes),
            dependency_manifest: reference("dependency.json", &bytes),
            files: vec![reference("generated.c", b"int value;\n")],
        };
        let encoded = format.encode_cache(&cache).unwrap();
        let decoded = format.decode_cache(&encoded).unwrap();
        assert_eq!(decoded.input_key, cache.input_key);
        assert_eq!(decoded.dependency_manifest, cache.dependency_manifest);
        assert_eq!(decoded.files, cache.files);
        let mut malformed: Value = serde_json::from_slice(&encoded).unwrap();
        malformed["schema"] = json!("madb.cache-entry/99");
        assert!(
            format
                .decode_cache(&serde_json::to_vec(&malformed).unwrap())
                .is_err()
        );
        let mut malformed: Value = serde_json::from_slice(&bytes).unwrap();
        malformed["complete"] = json!(true);
        assert!(
            format
                .decode_dependencies(&serde_json::to_vec(&malformed).unwrap())
                .is_err()
        );
    }
}
