//! Public E02 author support around the existing ordinary C/ArkTS generator.
//! The outer Host owns the process group and final artifact admission.
#[path = "arkts_composition_peer/business.rs"]
mod business;

use madb_protocol::{
    ExtensionFrame,
    extension_author::{AuthorEvent, AuthorSession, read_frame, write_frame},
};
use serde_json::json;
use std::{
    io,
    sync::mpsc::{self, Receiver},
    thread,
};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
const PACKAGE_ROOT: &str = "inputs/packages/madb.local.c07-composition/0.1.0";
const COMPONENT: &str = "madb.local.c07-composition/generator";

fn main() -> Result<()> {
    if std::env::args_os().len() != 1 {
        return Err("ordinary generator peer takes no arguments".into());
    }
    let manifest = business::read_relative(&format!("{PACKAGE_ROOT}/extension.json"), 1024 * 1024)?;
    let mut session = AuthorSession::from_manifest(&manifest, "generator")?;
    let incoming = frames();
    let mut writer = io::stdout().lock();
    let hello = receive(&incoming)?;
    let AuthorEvent::Reply(reply) = session.receive(hello)? else {
        return Err("expected initial handshake".into());
    };
    write_frame(&mut writer, &reply)?;
    if reply.value()["kind"] == "reject" {
        return Ok(());
    }
    let AuthorEvent::Invoke(invoke) = session.receive(receive(&incoming)?)? else {
        return Err("expected ordinary generator invocation".into());
    };
    let result = business::run(invoke.value(), &incoming, &mut session);
    let terminal = match result {
        Ok(business::Outcome::Cancelled) => session.cancelled()?,
        Ok(business::Outcome::Generated(output)) => session.complete(output)?,
        Err(error) => session.failed(json!({
            "code":error.code, "stage":"generate", "message":error.to_string()
        }))?,
    };
    // Notification failure does not attempt a second terminal result.
    write_frame(&mut writer, &terminal)?;
    Ok(())
}

/// This bounded stdin pump has process lifetime. It never executes port work or
/// writes output files; OS process exit releases a blocked final read. A bounded
/// queue prevents an untrusted peer from accumulating arbitrary control frames.
fn frames() -> Receiver<std::result::Result<ExtensionFrame, String>> {
    let (send, receive) = mpsc::sync_channel(8);
    thread::spawn(move || {
        let mut input = io::stdin().lock();
        loop {
            let value = match read_frame(&mut input) {
                Ok(Some(frame)) => Ok(frame),
                Ok(None) => Err("control input reached EOF".to_owned()),
                Err(error) => Err(error.to_string()),
            };
            let last = value.is_err();
            if send.send(value).is_err() || last {
                return;
            }
        }
    });
    receive
}
fn receive(
    incoming: &Receiver<std::result::Result<ExtensionFrame, String>>,
) -> Result<ExtensionFrame> {
    incoming.recv()?.map_err(Into::into)
}
