//! Controlled E02 peer. Protocol mechanics use only the public support module;
//! the fixture port produces bounded BuildEvidence, not a real compiler result.
use madb_protocol::{
    DocumentKind, ExtensionFrame, SchemaRegistry, byte_digest,
    extension_author::{AuthorEvent, AuthorSession, read_frame, write_frame},
};
use serde_json::json;
use std::{
    fs,
    io::{self, Write},
    path::Path,
    process::Command,
    thread,
    time::Duration,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("--child") {
        loop {
            thread::sleep(Duration::from_secs(1));
        }
    }
    let mode = args.get(1).map(String::as_str).unwrap_or("ok");
    let manifest = fs::read(Path::new(&args[0]).parent().unwrap().join("manifest.json"))?;
    let manifest_value: serde_json::Value = serde_json::from_slice(&manifest)?;
    let component = manifest_value["components"][0]["id"]
        .as_str()
        .ok_or("component id")?;
    let mut author = AuthorSession::from_manifest(&manifest, component)?;
    let mut input = io::stdin().lock();
    let mut output = io::stdout().lock();
    let hello = read_frame(&mut input)?.ok_or("missing hello")?;
    if mode == "crash-before-ack" {
        std::process::exit(23);
    }
    if mode == "handshake-timeout" {
        loop {
            thread::sleep(Duration::from_secs(1));
        }
    }
    if mode == "noise" {
        output.write_all(b"not a protocol frame\n")?;
        output.flush()?;
        return Ok(());
    }
    if mode == "utf8" {
        output.write_all(b"\xff\n")?;
        output.flush()?;
        return Ok(());
    }
    if mode == "oversize" {
        output.write_all(&vec![b'x'; 1024 * 1024 + 1])?;
        output.flush()?;
        return Ok(());
    }
    if mode == "duplicate-key" {
        output.write_all(b"{\"kind\":\"hello_ack\",\"kind\":\"hello_ack\"}\n")?;
        output.flush()?;
        return Ok(());
    }
    let AuthorEvent::Reply(ack) = author.receive(hello)? else {
        return Err("missing acknowledgement".into());
    };
    if mode == "cancel-before-ack" {
        fs::write("outputs/request-1/ready", b"ready")?;
        if read_frame(&mut input)?.is_some() {
            return Err("invoked before acknowledgement".into());
        }
        write_frame(&mut output, &ack)?;
        return Ok(());
    }
    let mut ack_value = ack.value().clone();
    match mode {
        "wrong-identity" => ack_value["extension_version"] = json!("9.0.0"),
        "wrong-session" => ack_value["session_id"] = json!("other-session"),
        "wrong-version" => ack_value["protocol"] = json!("9.0"),
        "extra-feature" => ack_value["features"] = json!(["unknown.extra@1"]),
        _ => {}
    }
    let ack = ExtensionFrame::from_value(ack_value)?;
    if mode == "early-result" {
        let result = ExtensionFrame::from_value(
            json!({"schema":"madb.extension-wire/1","kind":"result","session_id":"session-1","protocol":"1.0","ir":"1.0","request_id":"request-1","status":"ok","output":{"path":"outputs/request-1/result.json","schema":"madb.build-evidence/1","sha256":format!("sha256:{}","1".repeat(64)),"size":1}}),
        )?;
        let mut bytes = ack.encode_line()?;
        bytes.extend(result.encode_line()?);
        output.write_all(&bytes)?;
        output.flush()?;
        return Ok(());
    }
    write_frame(&mut output, &ack)?;
    let Some(invoke) = read_frame(&mut input)? else {
        return Ok(());
    };
    let AuthorEvent::Invoke(invoke) = author.receive(invoke)? else {
        return Err("missing invocation".into());
    };
    let request = invoke.value();
    let payload_bytes = fs::read(request["input"]["path"].as_str().ok_or("input path")?)?;
    if byte_digest(&payload_bytes) != request["input"]["sha256"]
        || payload_bytes.len() as u64 != request["input"]["size"].as_u64().ok_or("size")?
    {
        return Err("input identity mismatch".into());
    }
    let registry = SchemaRegistry::new()?;
    let payload = registry.decode(DocumentKind::BuildTask, &payload_bytes)?;
    // The author checks the same actual declared input bytes independently of
    // the host. These logical paths have already passed the public schema.
    for reference in payload.value()["inputs"].as_array().ok_or("inputs")? {
        let path = Path::new("inputs").join(reference["path"].as_str().ok_or("input path")?);
        let bytes = fs::read(path)?;
        if byte_digest(&bytes) != reference["sha256"]
            || bytes.len() as u64 != reference["size"].as_u64().ok_or("input size")?
        {
            return Err("declared input bytes differ".into());
        }
    }
    if mode == "tool-error" {
        write_frame(&mut output,&author.failed(json!({"code":"CustomCompilerFailure","message":"controlled error","stage":"build","source":"fixture.c:7"}))?)?;
        return Ok(());
    }
    if mode == "crash" {
        std::process::exit(23);
    }
    if mode == "eof" {
        return Ok(());
    }
    if matches!(mode, "cancel" | "cancel-ok" | "timeout" | "ignore-cancel") {
        fs::write("outputs/request-1/ready", b"ready")?;
        if matches!(mode, "timeout" | "ignore-cancel") {
            loop {
                thread::sleep(Duration::from_secs(1));
            }
        }
        let cancel = read_frame(&mut input)?.ok_or("missing cancel")?;
        if !matches!(author.receive(cancel)?, AuthorEvent::Cancel) {
            return Err("missing cancellation event".into());
        }
        if mode == "cancel" {
            write_frame(&mut output, &author.cancelled()?)?;
            return Ok(());
        }
    }
    let root = Path::new(request["output_root"].as_str().ok_or("output root")?);
    let data = b"fixture";
    fs::write(root.join("fixture.o"), data)?;
    let log = b"Controlled protocol fixture; no compiler invocation.\n";
    fs::write(root.join("build.log"), log)?;
    let p = payload.value();
    let mut evidence = json!({"schema":"madb.build-evidence/1", "task_id":p["id"], "task_digest":p["task_digest"], "input_digest":p["input_digest"], "profile_digest":p["selector"]["profile_digest"], "generation_id":p["generation_id"], "tool":p["tool"], "phase":p["phase"], "target":p["selector"]["target"], "status":"passed", "exit_code":0, "outputs":[{"path":"fixture.o","size":data.len(),"sha256":byte_digest(data)}], "logs":[{"path":"build.log","sha256":byte_digest(log),"size":log.len()}]});
    if mode == "business-failed" {
        evidence["status"] = json!("failed");
        evidence["exit_code"] = json!(1);
    }
    if mode == "wrong-payload" {
        evidence["task_digest"] = json!(format!("sha256:{}", "0".repeat(64)));
    }
    let bytes = serde_json::to_vec(&evidence)?;
    if mode != "late-create" {
        fs::write(root.join("result.json"), &bytes)?;
    }
    let mut reference = json!({"path":format!("{}/result.json",root.display()),"schema":"madb.build-evidence/1","sha256":byte_digest(&bytes),"size":bytes.len()});
    if mode == "outside-output" {
        reference["path"] = json!("inputs/request.json");
    }
    if mode == "wrong-digest" {
        reference["sha256"] = json!(format!("sha256:{}", "0".repeat(64)));
    }
    if mode == "undeclared" {
        fs::write(root.join("extra.txt"), b"extra")?;
    }
    if mode == "descendant" {
        let child = Command::new(&args[0]).arg("--child").spawn()?;
        eprintln!("descendant={}", child.id());
    }
    let result = author.complete(reference)?;
    let mut value = result.value().clone();
    if mode == "wrong-request" {
        value["request_id"] = json!("other-request");
    }
    if mode == "wrong-result-version" {
        value["ir"] = json!("9.0");
    }
    let result = ExtensionFrame::from_value(value)?;
    write_frame(&mut output, &result)?;
    if mode == "duplicate-result" {
        write_frame(&mut output, &result)?;
    }
    if mode == "crash-after-result" {
        std::process::exit(23);
    }
    if mode == "no-exit" {
        loop {
            thread::sleep(Duration::from_secs(1));
        }
    }
    if matches!(mode, "slow-observer-phase" | "slow-observer-global") {
        return Ok(());
    }
    if read_frame(&mut input)?.is_some() {
        return Err("single-request peer received another message".into());
    }
    if mode == "late-write" {
        fs::write(root.join("fixture.o"), b"changed")?;
    }
    if mode == "late-create" {
        fs::write(root.join("result.json"), &bytes)?;
    }
    Ok(())
}
