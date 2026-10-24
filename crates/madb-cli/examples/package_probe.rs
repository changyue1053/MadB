//! Test-only composition root for real transactional stores and process barriers.
//! Fixtures supply independent file identities. This is not a public CLI format.
use madb_app::{
    Digest, ErrorKind, LogicalPath, QueryError, Result,
    inputs::capture_inputs,
    ports::FileReference,
    transaction::{Batch, BatchSpec, CommitControl, CommitDisposition, IndexSnapshot, Layer},
};
use madb_host::{
    LocalWorkspace,
    transaction::{LocalBatchStore, TransactionPoint},
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
fn digest(value: &Value) -> Digest {
    Digest::new(value.as_str().expect("fixture digest")).expect("valid fixture digest")
}
fn optional_digest(value: &Value) -> Option<Digest> {
    (!value.is_null()).then(|| digest(value))
}
fn layer(value: &Value) -> Layer {
    match value.as_str().expect("fixture layer") {
        "source" => Layer::Source,
        "build" => Layer::Build,
        "candidate" => Layer::Candidate,
        _ => panic!("unknown fixture layer"),
    }
}
fn snapshot(value: &IndexSnapshot) -> Value {
    json!({"revision":value.revision,"source":value.source.as_ref().map(Digest::as_str),
        "build":value.build.as_ref().map(Digest::as_str),"candidate":value.candidate.as_ref().map(Digest::as_str)})
}
fn expected(value: &Value) -> IndexSnapshot {
    IndexSnapshot {
        revision: value["revision"].as_u64().expect("fixture revision"),
        source: optional_digest(&value["source"]),
        build: optional_digest(&value["build"]),
        candidate: optional_digest(&value["candidate"]),
    }
}
fn io_error(error: impl std::fmt::Display) -> QueryError {
    QueryError::new(
        ErrorKind::Host,
        "TEST_DRIVER_IO",
        "fixture",
        error.to_string(),
    )
}
fn point_name(point: TransactionPoint) -> &'static str {
    match point {
        TransactionPoint::StageCreated => "StageCreated",
        TransactionPoint::FileWritten => "FileWritten",
        TransactionPoint::FileSynced => "FileSynced",
        TransactionPoint::Staged => "Staged",
        TransactionPoint::BeforeIndexReplace => "BeforeIndexReplace",
        TransactionPoint::AfterIndexReplace => "AfterIndexReplace",
    }
}
fn wait_for_release(config: &Value, point: &str) -> Result<()> {
    let ready = Path::new(text(config, "ready"));
    fs::write(
        ready,
        serde_json::to_vec(&json!({"point":point,"pid":std::process::id()})).unwrap(),
    )
    .map_err(io_error)?;
    let release = Path::new(text(config, "release"));
    let start = Instant::now();
    while !release.exists() {
        if start.elapsed() >= Duration::from_secs(12) {
            return Err(QueryError::new(
                ErrorKind::Host,
                "TEST_BARRIER_TIMEOUT",
                point,
                "fixture release was not received",
            ));
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    Ok(())
}
struct ProbeHook<'a> {
    config: &'a Value,
    control: &'a CommitControl,
    paused: bool,
    cancelled: bool,
    decisions: Vec<String>,
}
impl ProbeHook<'_> {
    fn visit(&mut self, point: TransactionPoint) -> Result<()> {
        let point = point_name(point);
        if !self.cancelled && self.config["cancel_at"].as_str() == Some(point) {
            self.cancelled = true;
            self.decisions.push(format!("{:?}", self.control.cancel()));
        }
        if !self.paused && self.config["gate_at"].as_str() == Some(point) {
            self.paused = true;
            wait_for_release(self.config, point)?;
        }
        Ok(())
    }
}
fn batch(config: &Value) -> Result<Batch> {
    let workspace = LocalWorkspace::open(Path::new(text(config, "input_root")))?;
    let references = config["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| FileReference {
            path: LogicalPath::new(text(value, "path")).unwrap(),
            digest: digest(&value["sha256"]),
            size: value["size"].as_u64().unwrap(),
        })
        .collect();
    let files = capture_inputs(&workspace, references)?;
    let spec = &config["spec"];
    Batch::new(
        BatchSpec {
            layer: layer(&spec["layer"]),
            input_digest: digest(&spec["input_digest"]),
            task_digest: digest(&spec["task_digest"]),
            profile_digest: digest(&spec["profile_digest"]),
            generation_id: digest(&spec["generation_id"]),
            parent: optional_digest(&spec["parent"]),
        },
        files,
    )
}
fn verify_guard(config: &Value) -> Result<()> {
    if config["guard"].is_null() {
        return Ok(());
    }
    let guard = &config["guard"];
    let bytes = fs::read(text(guard, "path")).map_err(io_error)?;
    if Digest::of_bytes(&bytes) != digest(&guard["sha256"]) {
        return Err(QueryError::new(
            ErrorKind::Conflict,
            "TEST_INPUT_CHANGED",
            "guard",
            "source changed before commit",
        ));
    }
    Ok(())
}
fn commit(store: &LocalBatchStore, config: &Value) -> Result<Value> {
    let expected = if config["expected"].is_object() {
        expected(&config["expected"])
    } else {
        store.snapshot()?
    };
    let batch = batch(config)?;
    let content_id = batch.id().as_str().to_owned();
    let control = CommitControl::default();
    let mut hook = ProbeHook {
        config,
        control: &control,
        paused: false,
        cancelled: false,
        decisions: vec![],
    };
    if config["cancel_at"] == "before-stage" {
        hook.decisions.push(format!("{:?}", control.cancel()));
    }
    let staged = store.stage_with(&batch, |point| hook.visit(point))?;
    let staged_path = staged.path().to_owned();
    let staged_id = staged.id().as_str().to_owned();
    let result = store.commit_with(
        staged,
        &expected,
        &control,
        || verify_guard(config),
        |point| hook.visit(point),
    );
    let outcome = match result {
        Ok(receipt) => json!({"status":"ok","disposition":match receipt.disposition {
                CommitDisposition::Committed => "Committed", CommitDisposition::Reused => "Reused"},
            "index":snapshot(&receipt.index),"batch_id":receipt.batch_id.as_str()}),
        Err(error) => failure(&error),
    };
    Ok(
        json!({"outcome":outcome,"content_id":content_id,"staged_id":staged_id,
        "staged_path":staged_path,"cancellations":hook.decisions,"late_cancel":format!("{:?}",control.cancel()),
        "current":snapshot(&store.snapshot()?)}),
    )
}
fn pin(store: &LocalBatchStore, config: &Value) -> Result<Value> {
    let Some(lease) = store.pin(layer(&config["layer"]))? else {
        return Ok(json!({"missing":true}));
    };
    let path = lease.path().to_owned();
    let info = lease.info();
    let identity = json!({"id":info.id.as_str(),"content_id":info.content_id.as_str()});
    let before: Vec<_> = lease
        .files()
        .files()
        .map(|(r, bytes)| {
            json!({"path":r.path.as_str(),
        "sha256":r.digest.as_str(),"size":r.size,"bytes":bytes})
        })
        .collect();
    if config["ready"].is_string() {
        fs::write(
            text(config, "ready"),
            serde_json::to_vec(
                &json!({"point":"Pinned","path":path,"identity":identity,"pid":std::process::id()}),
            )
            .unwrap(),
        )
        .map_err(io_error)?;
        let start = Instant::now();
        while !Path::new(text(config, "release")).exists() {
            if start.elapsed() >= Duration::from_secs(12) {
                return Err(io_error("pin release deadline"));
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    let after: Vec<_> = lease
        .files()
        .files()
        .map(|(r, bytes)| {
            json!({"path":r.path.as_str(),
        "sha256":r.digest.as_str(),"size":r.size,"bytes":bytes})
        })
        .collect();
    Ok(
        json!({"missing":false,"path":path,"exists":path.is_dir(),"identity":identity,"before":before,"after":after}),
    )
}
fn failure(error: &QueryError) -> Value {
    json!({"status":"error","kind":format!("{:?}",error.kind),
        "codes":error.findings.iter().map(|f|f.code).collect::<Vec<_>>(),"message":error.to_string()})
}
fn run(config: &Value) -> Result<Value> {
    let store = LocalBatchStore::open(Path::new(text(config, "domain")))?;
    match text(config, "operation") {
        "commit" => commit(&store, config),
        "snapshot" => Ok(json!({"index":snapshot(&store.snapshot()?)})),
        "pin" => pin(&store, config),
        "recover" => {
            let report = store.recover()?;
            Ok(
                json!({"removed_staging":report.removed_staging,"removed_batches":report.removed_batches,
                "retained_active":report.retained_active,"index":snapshot(&store.snapshot()?)}),
            )
        }
        _ => panic!("unknown fixture operation"),
    }
}
fn main() {
    let config: Value = serde_json::from_slice(
        &fs::read(std::env::args().nth(1).expect("fixture config")).unwrap(),
    )
    .unwrap();
    let result = match run(&config) {
        Ok(value) => value,
        Err(error) => failure(&error),
    };
    println!("{}", result);
}
