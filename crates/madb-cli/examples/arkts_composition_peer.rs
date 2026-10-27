//! Public E02 author support around the fixed C/ArkTS reader and generator.
//! The outer Host owns the process group and final artifact admission.
#[path = "arkts_composition_peer/business.rs"]
mod business;

use madb_protocol::{
    DocumentKind, ExtensionFrame,
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
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Operation {
    Read,
    Generate,
}
impl Operation {
    fn select(arguments: &[String]) -> Result<Self> {
        match arguments {
            [] => Ok(Self::Generate),
            [name] if name == "generate" => Ok(Self::Generate),
            [name] if name == "read" => Ok(Self::Read),
            _ => Err("peer accepts only one fixed read or generate argument".into()),
        }
    }
    fn name(self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::Generate => "generate",
        }
    }
    fn component(self) -> &'static str {
        match self {
            Self::Read => "reader",
            Self::Generate => "generator",
        }
    }
    fn request_kind(self) -> DocumentKind {
        match self {
            Self::Read => DocumentKind::ReadRequest,
            Self::Generate => DocumentKind::GenerateRequest,
        }
    }
    fn result_kind(self) -> DocumentKind {
        match self {
            Self::Read => DocumentKind::ContractIr,
            Self::Generate => DocumentKind::OutputSet,
        }
    }
    fn request_file(self) -> &'static str {
        match self {
            Self::Read => "read-request.json",
            Self::Generate => "generate-request.json",
        }
    }
    fn result_file(self) -> &'static str {
        match self {
            Self::Read => "contract.json",
            Self::Generate => "outputs.json",
        }
    }
}

fn main() -> Result<()> {
    let operation = Operation::select(&std::env::args().skip(1).collect::<Vec<_>>())?;
    let manifest = business::read_relative(&format!("{PACKAGE_ROOT}/extension.json"), 1024 * 1024)?;
    let mut session = AuthorSession::from_manifest(&manifest, operation.component())?;
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
        return Err("expected selected component invocation".into());
    };
    let result = business::run(operation, invoke.value(), &incoming, &mut session);
    let terminal = match result {
        Ok(business::Outcome::Cancelled) => session.cancelled()?,
        Ok(business::Outcome::Generated(output)) => session.complete(output)?,
        Err(error) => session.failed(json!({
            "code":error.code, "stage":operation.name(), "message":error.to_string()
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
