//! Fixed two-component protocol fixture using the existing storage transaction.
//! This is not the E03 route/responsibility planner or a new product command.
#[path = "extension_session_probe/session.rs"]
mod session;

use madb_app::{
    Digest, LogicalPath,
    inputs::{FrozenInputs, capture_inputs},
    ports::FileReference,
    transaction::{Batch, BatchSpec, CommitControl, Layer},
};
use madb_host::{LocalWorkspace, transaction::LocalBatchStore};
use serde_json::{Value, json};
use std::{fs, path::Path};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    let peer = Path::new(&args[1]);
    let mode = args[2].as_str();
    let root = Path::new(&args[3]);
    fs::create_dir(root)?;
    fs::create_dir(root.join("baseline"))?;
    let bytes = b"controlled source baseline";
    fs::write(root.join("baseline/source.txt"), bytes)?;
    let baseline = capture_inputs(
        &LocalWorkspace::open(&root.join("baseline"))?,
        vec![FileReference {
            path: LogicalPath::new("source.txt")?,
            digest: Digest::of_bytes(bytes),
            size: bytes.len() as u64,
        }],
    )?;
    let store = LocalBatchStore::open(&root.join("store"))?;
    let source = Batch::new(spec(Layer::Source, None), baseline)?;
    let baseline_receipt = store.commit(
        store.stage(&source)?,
        &store.snapshot()?,
        &CommitControl::default(),
        || Ok(()),
    )?;
    let before = baseline_receipt.index;
    let first_mode = if mode == "first-failed" {
        "business-failed"
    } else {
        "ok"
    };
    let (first_report, first) = session::run(peer, first_mode, &root.join("first"), "first", None)?;
    let mut reports = vec![first_report];
    let mut transferred = None;
    let mut commit_error = None;
    if let Some(first) = first {
        if mode == "upstream-mutated" {
            // An already admitted immutable value cannot change when the old
            // helper directory changes; downstream must receive the old bytes.
            fs::write(
                root.join("first/session/work/outputs/request-1/fixture.o"),
                b"tampered",
            )?;
        }
        let second_mode = match mode {
            "second-failed" => "business-failed",
            "second-cancel" => "cancel",
            "second-late-write" => "late-write",
            _ => "ok",
        };
        let (second_report, second) = session::run(
            peer,
            second_mode,
            &root.join("second"),
            "second",
            Some(&first),
        )?;
        reports.push(second_report);
        let request: Value =
            serde_json::from_slice(&fs::read(root.join("second/source/request.json"))?)?;
        transferred = Some(json!({
            "references":request["inputs"],
            "expected":first.references().map(|r|json!({"path":r.path.as_str(),"sha256":r.digest.as_str(),"size":r.size})).collect::<Vec<_>>(),
            "fixture_bytes":fs::read(root.join("second/source/fixture.o"))?
        }));
        if let Some(second) = second {
            let frozen = combine(root, &[(&first, "first"), (&second, "second")])?;
            let batch = Batch::new(spec(Layer::Build, Some(baseline_receipt.batch_id)), frozen)?;
            let control = CommitControl::default();
            if mode == "cancel-commit" {
                control.cancel();
            }
            if let Err(error) = store.commit(store.stage(&batch)?, &before, &control, || Ok(())) {
                commit_error = Some(error.to_string());
            }
        }
    }
    let after = store.snapshot()?;
    println!(
        "{}",
        json!({
            "reports":reports,"transferred":transferred,"committed":after.build.is_some(),
            "index_unchanged":after==before,"before_revision":before.revision,"after_revision":after.revision,
            "commit_error":commit_error
        })
    );
    Ok(())
}

fn spec(layer: Layer, parent: Option<Digest>) -> BatchSpec {
    BatchSpec {
        layer,
        parent,
        input_digest: Digest::of_bytes(b"controlled source identity"),
        task_digest: Digest::of_bytes(b"fixed two component protocol probe"),
        profile_digest: Digest::of_bytes(b"controlled profile"),
        generation_id: Digest::of_bytes(b"controlled generation"),
    }
}

fn combine(
    root: &Path,
    components: &[(&FrozenInputs, &str)],
) -> Result<FrozenInputs, Box<dyn std::error::Error>> {
    let destination = root.join("combined");
    fs::create_dir(&destination)?;
    let mut references = Vec::new();
    for (files, component) in components {
        for (reference, bytes) in files.files() {
            let path = LogicalPath::new(format!("{component}/{}", reference.path.as_str()))?;
            let disk = destination.join(path.as_str());
            fs::create_dir_all(disk.parent().unwrap())?;
            fs::write(&disk, bytes)?;
            references.push(FileReference {
                path,
                digest: reference.digest.clone(),
                size: reference.size,
            });
        }
    }
    Ok(capture_inputs(
        &LocalWorkspace::open(&destination)?,
        references,
    )?)
}
