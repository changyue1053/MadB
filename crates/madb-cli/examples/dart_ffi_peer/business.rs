use super::{ExtensionFrame, Operation, PACKAGE_ROOT, Result};
use madb_protocol::{
    SchemaRegistry, Value, byte_digest,
    extension_author::{AuthorEvent, AuthorSession},
    strict_json,
};
use serde_json::json;
use std::{
    fs::{self, File},
    io::{Read, Write},
    path::{Component, Path, PathBuf},
    process::{Command, ExitStatus, Stdio},
    sync::mpsc::{self, Receiver, TryRecvError},
    thread,
    time::Duration,
};

const DOCUMENT_LIMIT: usize = 1024 * 1024;
const IMAGE_LIMIT: usize = 256 * 1024 * 1024;
const QUANTUM: Duration = Duration::from_millis(10);
const PYTHON_ARGS: &[&str] = &["-I", "-S", "-B", "-X", "pycache_prefix=/dev/null"];

pub enum Outcome {
    Generated(Value),
    Cancelled,
}
#[derive(Debug)]
pub struct Failure {
    pub code: &'static str,
    message: String,
}
impl Failure {
    fn new(code: &'static str, error: impl std::fmt::Display) -> Self {
        Self {
            code,
            message: error.to_string(),
        }
    }
}
impl std::fmt::Display for Failure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}
impl std::error::Error for Failure {}

struct Prepared {
    operation: Operation,
    request_path: String,
    request_bytes: Vec<u8>,
    config_bytes: Vec<u8>,
    python_path: PathBuf,
    python_digest: String,
    python_size: usize,
    script_path: PathBuf,
    input_root: PathBuf,
    output_root: PathBuf,
    output_name: String,
}

pub fn run(
    operation: Operation,
    invoke: &Value,
    incoming: &Receiver<std::result::Result<ExtensionFrame, String>>,
    session: &mut AuthorSession,
) -> std::result::Result<Outcome, Failure> {
    let prepared = prepare(operation, invoke).map_err(|e| Failure::new("InputInvalid", e))?;
    let mut cancelled = false;
    receive_pending(incoming, session, &mut cancelled)?;
    if cancelled {
        return Ok(Outcome::Cancelled);
    }
    // Do not call setpgid/start_new_session: the outer Host must retain all
    // derived work in its own group, including timeout/crash cleanup. This
    // adapter intentionally does not create a second process supervisor.
    let mut child = Command::new(&prepared.python_path)
        .args(PYTHON_ARGS)
        .arg(&prepared.script_path)
        .arg("--operation")
        .arg(operation.name())
        .arg("--input-root")
        .arg(&prepared.input_root)
        .arg("--output-root")
        .arg(&prepared.output_root)
        .arg("--request")
        .arg(&prepared.request_path)
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| Failure::new("ToolFailed", e))?;
    let (send, streams) = mpsc::channel();
    let stdout = child.stdout.take().expect("requested stdout pipe");
    let stderr = child.stderr.take().expect("requested stderr pipe");
    let out_send = send.clone();
    let out_thread = thread::spawn(move || {
        let _ = out_send.send((true, read_bounded(stdout)));
    });
    let err_thread = thread::spawn(move || {
        let _ = send.send((false, read_bounded(stderr)));
    });
    let mut exit: Option<ExitStatus> = None;
    let mut out = None;
    let mut err = None;
    let mut protocol_failure = None;
    // An accepted cancel forbids the result, but does not claim the compiler
    // stopped immediately. Only reaped child + both EOFs allow cancelled();
    // otherwise the outer Host budget terminates the entire inherited group.
    while exit.is_none() || out.is_none() || err.is_none() {
        if protocol_failure.is_none()
            && let Err(error) = receive_pending(incoming, session, &mut cancelled)
        {
            protocol_failure = Some(error);
        }
        if exit.is_none() {
            match child.try_wait() {
                Ok(status) => exit = status,
                Err(error) => {
                    // Keep ownership and wait. A wait failure cannot certify
                    // cleanup; returning would leave the subprocess behind.
                    protocol_failure.get_or_insert_with(|| Failure::new("ToolFailed", error));
                }
            }
        }
        while let Ok((is_stdout, bytes)) = streams.try_recv() {
            if is_stdout {
                out = Some(bytes);
            } else {
                err = Some(bytes);
            }
        }
        if exit.is_none() || out.is_none() || err.is_none() {
            thread::sleep(QUANTUM);
        }
    }
    out_thread
        .join()
        .map_err(|_| Failure::new("ToolFailed", "stdout reader panicked"))?;
    err_thread
        .join()
        .map_err(|_| Failure::new("ToolFailed", "stderr reader panicked"))?;
    let out = out
        .expect("completed stdout")
        .map_err(|e| Failure::new("ToolFailed", e))?;
    let err = err
        .expect("completed stderr")
        .map_err(|e| Failure::new("ToolFailed", e))?;
    std::io::stderr()
        .write_all(&err.bytes)
        .map_err(|e| Failure::new("ToolFailed", e))?;
    if let Some(error) = protocol_failure {
        return Err(error);
    }
    receive_pending(incoming, session, &mut cancelled)?;
    if cancelled {
        return Ok(Outcome::Cancelled);
    }
    if out.overflow || err.overflow {
        return Err(Failure::new(
            "ToolFailed",
            "business process output exceeded budget",
        ));
    }
    if !exit.expect("reaped child").success() {
        return Err(Failure::new(
            "ToolFailed",
            "selected component business process failed",
        ));
    }
    let result =
        validate_result(&prepared, &out.bytes).map_err(|e| Failure::new("OutputInvalid", e))?;
    receive_pending(incoming, session, &mut cancelled)?;
    if cancelled {
        Ok(Outcome::Cancelled)
    } else {
        Ok(Outcome::Generated(result))
    }
}

fn receive_pending(
    incoming: &Receiver<std::result::Result<ExtensionFrame, String>>,
    session: &mut AuthorSession,
    cancelled: &mut bool,
) -> std::result::Result<(), Failure> {
    loop {
        match incoming.try_recv() {
            Ok(Ok(frame)) => match session.receive(frame) {
                Ok(AuthorEvent::Cancel) => *cancelled = true,
                Ok(_) => return Err(Failure::new("ProtocolInvalid", "unexpected control event")),
                Err(error) => return Err(Failure::new("ProtocolInvalid", error)),
            },
            Ok(Err(error)) => return Err(Failure::new("ProtocolInvalid", error)),
            Err(TryRecvError::Empty) => return Ok(()),
            Err(TryRecvError::Disconnected) => {
                return Err(Failure::new(
                    "ProtocolInvalid",
                    "control input disconnected",
                ));
            }
        }
    }
}

struct BoundedBytes {
    bytes: Vec<u8>,
    overflow: bool,
}
fn read_bounded(mut stream: impl Read) -> std::io::Result<BoundedBytes> {
    let mut bytes = Vec::new();
    let mut buffer = [0; 8192];
    let mut overflow = false;
    loop {
        let count = stream.read(&mut buffer)?;
        if count == 0 {
            return Ok(BoundedBytes { bytes, overflow });
        }
        let retain = count.min(DOCUMENT_LIMIT.saturating_sub(bytes.len()));
        bytes.extend_from_slice(&buffer[..retain]);
        overflow |= retain != count;
    }
}

fn prepare(operation: Operation, invoke: &Value) -> Result<Prepared> {
    let component = format!("org.madb.dart-ffi/{}", operation.component());
    if invoke["operation"] != operation.name() || invoke["component"] != component {
        return Err("peer only serves its selected fixed component".into());
    }
    let input = invoke["input"]["path"]
        .as_str()
        .ok_or("missing request path")?;
    if input != format!("inputs/{}", operation.request_file())
        || invoke["input"]["schema"] != operation.request_kind().schema_id()
    {
        return Err("peer requires the selected request schema and fixed location".into());
    }
    let request_bytes = read_relative(input, DOCUMENT_LIMIT)?;
    check_reference(&invoke["input"], &request_bytes)?;
    let request = SchemaRegistry::new()?.decode(operation.request_kind(), &request_bytes)?;
    if request.value()["component"]["id"] != component
        || request.value()["profile"]["schema"] != "madb.profile/2"
        || request.value()["output_root"] != invoke["output_root"]
    {
        return Err("request component, profile or output domain differs".into());
    }
    let output_name = invoke["output_root"]
        .as_str()
        .ok_or("missing output root")?;
    validate_relative(output_name)?;
    if !output_name.starts_with("outputs/") || output_name.split('/').count() != 2 {
        return Err("Dart peer requires outputs/request domain".into());
    }
    let cwd = std::env::current_dir()?;
    let output_root = cwd.join(output_name);
    check_path(&output_root, true)?;
    if fs::read_dir(&output_root)?.next().is_some() {
        return Err("output domain is not empty".into());
    }
    let config_bytes = read_relative("inputs/backend.json", DOCUMENT_LIMIT)?;
    let config = strict_json(&config_bytes)?;
    let python = &config["tools"]["python"];
    if python["id"] != "e04-python" || python["invocation_arguments"] != json!(PYTHON_ARGS) {
        return Err("Dart peer requires the fixed isolated Python invocation".into());
    }
    let python_path = PathBuf::from(python["path"].as_str().ok_or("missing Python path")?);
    if !python_path.is_absolute() {
        return Err("Python image must use the host's explicit installed path".into());
    }
    let python_size = python["size"].as_u64().ok_or("missing Python size")?;
    if python_size == 0 || python_size > IMAGE_LIMIT as u64 {
        return Err("Python image exceeds budget".into());
    }
    let python_bytes = read_file(&python_path, python_size as usize)?;
    check_reference(python, &python_bytes)?;
    let script_path = cwd.join(format!(
        "{PACKAGE_ROOT}/adapters/generators/dart_ffi/peer.py"
    ));
    check_path(&script_path, false)?;
    Ok(Prepared {
        operation,
        request_path: operation.request_file().into(),
        request_bytes,
        config_bytes,
        python_path,
        python_digest: byte_digest(&python_bytes),
        python_size: python_bytes.len(),
        script_path,
        input_root: cwd.join("inputs"),
        output_root,
        output_name: output_name.to_owned(),
    })
}
fn validate_result(prepared: &Prepared, stdout: &[u8]) -> Result<Value> {
    if read_relative(&format!("inputs/{}", prepared.request_path), DOCUMENT_LIMIT)?
        != prepared.request_bytes
        || read_relative("inputs/backend.json", DOCUMENT_LIMIT)? != prepared.config_bytes
        || byte_digest(&read_file(&prepared.python_path, prepared.python_size)?)
            != prepared.python_digest
    {
        return Err("request, backend or Python image changed during execution".into());
    }
    let output = SchemaRegistry::new()?.decode(prepared.operation.result_kind(), stdout)?;
    let result_file = prepared.operation.result_file();
    if prepared.operation == Operation::Generate && output.value()["manifest_path"] != result_file {
        return Err("output manifest location differs".into());
    }
    if prepared.operation == Operation::Read {
        let mut files = fs::read_dir(&prepared.output_root)?;
        if files.next().transpose()?.map(|entry| entry.file_name()) != Some(result_file.into())
            || files.next().transpose()?.is_some()
        {
            return Err("reader output must contain only contract.json".into());
        }
    }
    let path = prepared.output_root.join(result_file);
    if read_file(&path, DOCUMENT_LIMIT)? != stdout {
        return Err("stdout and actual output manifest bytes differ".into());
    }
    Ok(
        json!({"path":format!("{}/{result_file}",prepared.output_name),
        "schema":prepared.operation.result_kind().schema_id(),"sha256":byte_digest(stdout),"size":stdout.len()}),
    )
}
fn check_reference(reference: &Value, bytes: &[u8]) -> Result<()> {
    if reference["sha256"] != byte_digest(bytes)
        || reference["size"].as_u64() != Some(bytes.len() as u64)
    {
        return Err("referenced file byte identity differs".into());
    }
    Ok(())
}
fn validate_relative(name: &str) -> Result<()> {
    if name.is_empty()
        || !name.is_ascii()
        || name.contains(['\\', '\0'])
        || name.split('/').count() > 32
        || Path::new(name)
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
        || name
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err("invalid confined relative path".into());
    }
    Ok(())
}
pub fn read_relative(name: &str, limit: usize) -> Result<Vec<u8>> {
    validate_relative(name)?;
    read_file(&std::env::current_dir()?.join(name), limit)
}
fn read_file(path: &Path, limit: usize) -> Result<Vec<u8>> {
    check_path(path, false)?;
    let mut bytes = Vec::new();
    File::open(path)?
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > limit {
        return Err("file exceeds read budget".into());
    }
    Ok(bytes)
}
fn check_path(path: &Path, directory: bool) -> Result<()> {
    for ancestor in path.ancestors() {
        if fs::symlink_metadata(ancestor)?.file_type().is_symlink() {
            return Err("symbolic file or ancestor is not allowed".into());
        }
    }
    let metadata = fs::symlink_metadata(path)?;
    if (directory && !metadata.is_dir()) || (!directory && !metadata.is_file()) {
        return Err("unexpected filesystem object".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if !directory && metadata.nlink() != 1 {
            return Err("hard-linked file is not allowed".into());
        }
    }
    Ok(())
}
