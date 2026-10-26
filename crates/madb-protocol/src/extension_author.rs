//! Public helper support for one component and one request per process.
//! Authors own port work and cleanup. cancelled() may be called only after
//! derived work and output writes have stopped; this library cannot prove it.
use crate::{
    ExtensionFrame, FormatError, MAX_EXTENSION_FRAME_BYTES, SchemaRegistry, Value, byte_digest,
};
use serde_json::json;
use std::io::{BufRead, Write};

enum State {
    Hello,
    Ready,
    Running,
    Cancelling,
    Finished,
    Broken,
}
pub enum AuthorEvent {
    Reply(ExtensionFrame),
    Invoke(ExtensionFrame),
    Cancel,
}
pub struct AuthorSession {
    manifest: Value,
    digest: String,
    component: Value,
    state: State,
    acknowledgement: Option<Value>,
    request: Option<Value>,
}
impl AuthorSession {
    pub fn from_manifest(bytes: &[u8], component_id: &str) -> Result<Self, FormatError> {
        let schema = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../docs/schemas/extension-manifest.schema.json"
        )))
        .map_err(|e| invalid("FMT000", format!("{e}")))?;
        let manifest = SchemaRegistry::validate_bundled(&schema, bytes)?;
        if manifest["example"] == true {
            return Err(invalid("InputInvalid", "design manifest cannot run"));
        }
        let components: Vec<_> = manifest["components"]
            .as_array()
            .expect("validated components")
            .iter()
            .filter(|c| c["id"] == component_id)
            .collect();
        if components.len() != 1 {
            return Err(invalid("InputInvalid", "component missing or ambiguous"));
        }
        let component = components[0].clone();
        Ok(Self {
            manifest,
            digest: byte_digest(bytes),
            component,
            state: State::Hello,
            acknowledgement: None,
            request: None,
        })
    }
    pub fn receive(&mut self, frame: ExtensionFrame) -> Result<AuthorEvent, FormatError> {
        let result = self.receive_checked(frame);
        if result.is_err() {
            self.state = State::Broken;
        }
        result
    }
    fn receive_checked(&mut self, frame: ExtensionFrame) -> Result<AuthorEvent, FormatError> {
        let value = frame.value();
        if matches!(self.state, State::Hello) {
            if value["kind"] != "hello" {
                return Err(invalid("ProtocolInvalid", "expected hello"));
            }
            let identity_mismatch = [
                ("extension_id", &self.manifest["id"]),
                ("extension_version", &self.manifest["version"]),
                ("component_id", &self.component["id"]),
            ]
            .iter()
            .any(|(key, expected)| &value[*key] != *expected);
            if identity_mismatch {
                return self.reject(
                    value,
                    "ExtensionIdentityMismatch",
                    "component identity differs",
                );
            }
            if value["manifest_digest"] != self.digest {
                return self.reject(value, "ExtensionIdentityMismatch", "manifest bytes differ");
            }
            let Some(protocol) = select_version(
                &value["protocol_versions"],
                &self.manifest["compatibility"]["protocol"],
            ) else {
                return self.reject(
                    value,
                    "ProtocolIncompatible",
                    "no declared protocol intersection",
                );
            };
            let Some(ir) =
                select_version(&value["ir_versions"], &self.manifest["compatibility"]["ir"])
            else {
                return self.reject(value, "ProtocolIncompatible", "no declared IR intersection");
            };
            let supported = self.component["supported_features"]
                .as_array()
                .expect("validated features");
            if value["required_features"]
                .as_array()
                .expect("validated required")
                .iter()
                .any(|f| !supported.contains(f))
            {
                return self.reject(
                    value,
                    "CapabilityUnsupported",
                    "required semantics are absent",
                );
            }
            let ack = json!({"schema":"madb.extension-wire/1", "kind":"hello_ack", "session_id":value["session_id"], "extension_id":self.manifest["id"], "extension_version":self.manifest["version"], "component_id":self.component["id"], "manifest_digest":self.digest, "protocol":protocol, "ir":ir, "features":self.component["supported_features"]});
            let frame = ExtensionFrame::from_value(ack.clone())?;
            self.acknowledgement = Some(ack);
            self.state = State::Ready;
            return Ok(AuthorEvent::Reply(frame));
        }
        let ack = self
            .acknowledgement
            .as_ref()
            .ok_or_else(|| invalid("ProtocolInvalid", "no successful handshake"))?;
        for key in ["session_id", "protocol", "ir"] {
            if value[key] != ack[key] {
                return Err(invalid(
                    "ProtocolInvalid",
                    "request does not belong to this negotiated session",
                ));
            }
        }
        match self.state {
            State::Ready => {
                if value["kind"] != "invoke" {
                    return Err(invalid("ProtocolInvalid", "expected invoke"));
                }
                let reference = format!(
                    "{}/{}",
                    self.manifest["id"].as_str().expect("id"),
                    self.component["id"].as_str().expect("id")
                );
                if value["component"] != reference
                    || Some(value["operation"].as_str().expect("operation"))
                        != operation(self.component["port"].as_str().expect("port"))
                {
                    return Err(invalid(
                        "CapabilityUnsupported",
                        "invocation targets another component or port",
                    ));
                }
                self.request = Some(value.clone());
                self.state = State::Running;
                Ok(AuthorEvent::Invoke(frame))
            }
            State::Running | State::Cancelling => {
                if value["kind"] != "cancel"
                    || value["request_id"]
                        != self.request.as_ref().expect("active request")["request_id"]
                {
                    return Err(invalid(
                        "ProtocolInvalid",
                        "only cancellation of the active request is allowed",
                    ));
                }
                self.state = State::Cancelling;
                Ok(AuthorEvent::Cancel)
            }
            _ => Err(invalid(
                "ProtocolInvalid",
                "unexpected message after terminal state",
            )),
        }
    }
    fn reject(
        &mut self,
        hello: &Value,
        code: &str,
        message: &str,
    ) -> Result<AuthorEvent, FormatError> {
        self.state = State::Finished;
        Ok(AuthorEvent::Reply(ExtensionFrame::from_value(
            json!({"schema":"madb.extension-wire/1", "kind":"reject", "session_id":hello["session_id"], "error":{"code":code,"message":message,"stage":"handshake"}}),
        )?))
    }
    pub fn complete(&mut self, output: Value) -> Result<ExtensionFrame, FormatError> {
        self.terminal("ok", Some(("output", output)))
    }
    pub fn failed(&mut self, diagnostic: Value) -> Result<ExtensionFrame, FormatError> {
        self.terminal("error", Some(("error", diagnostic)))
    }
    pub fn cancelled(&mut self) -> Result<ExtensionFrame, FormatError> {
        if !matches!(self.state, State::Cancelling) {
            return Err(invalid(
                "ProtocolInvalid",
                "cancelled requires an observed cancellation request",
            ));
        }
        self.terminal("cancelled", None)
    }
    fn terminal(
        &mut self,
        status: &str,
        payload: Option<(&str, Value)>,
    ) -> Result<ExtensionFrame, FormatError> {
        if !matches!(self.state, State::Running | State::Cancelling) {
            return Err(invalid(
                "ProtocolInvalid",
                "terminal result already sent or request not active",
            ));
        }
        let request = self.request.as_ref().expect("active request");
        let mut value = json!({"schema":"madb.extension-wire/1", "kind":"result", "session_id":request["session_id"], "protocol":request["protocol"], "ir":request["ir"], "request_id":request["request_id"], "status":status});
        if let Some((key, payload)) = payload {
            value[key] = payload;
        }
        let frame = ExtensionFrame::from_value(value)?;
        self.state = State::Finished;
        Ok(frame)
    }
}

/// Versions have already passed the envelope/manifest schema. Numeric segment
/// ordering uses digit length and bytes, so it cannot overflow a machine integer.
pub fn select_version(offered: &Value, supported: &Value) -> Option<String> {
    offered
        .as_array()?
        .iter()
        .filter(|v| supported.as_array().is_some_and(|s| s.contains(v)))
        .filter_map(Value::as_str)
        .filter(|v| version_key(v).is_some())
        .max_by(|a, b| version_key(a).cmp(&version_key(b)))
        .map(str::to_owned)
}
fn version_key(value: &str) -> Option<(usize, &str, usize, &str)> {
    let (major, minor) = value.split_once('.')?;
    if [major, minor].iter().any(|s| {
        s.is_empty()
            || !s.bytes().all(|b| b.is_ascii_digit())
            || (s.len() > 1 && s.starts_with('0'))
    }) {
        return None;
    }
    Some((major.len(), major, minor.len(), minor))
}
pub fn read_frame(reader: &mut impl BufRead) -> Result<Option<ExtensionFrame>, FormatError> {
    let mut line = Vec::new();
    loop {
        let available = reader
            .fill_buf()
            .map_err(|e| invalid("ToolFailed", e.to_string()))?;
        if available.is_empty() {
            return if line.is_empty() {
                Ok(None)
            } else {
                Err(invalid("ProtocolInvalid", "truncated line at EOF"))
            };
        }
        let size = available
            .iter()
            .position(|&b| b == b'\n')
            .map_or(available.len(), |n| n + 1);
        if line.len() + size > MAX_EXTENSION_FRAME_BYTES {
            return Err(invalid(
                "ProtocolInvalid",
                "control frame byte budget exceeded",
            ));
        }
        line.extend_from_slice(&available[..size]);
        reader.consume(size);
        if line.last() == Some(&b'\n') {
            return ExtensionFrame::decode_line(&line).map(Some);
        }
    }
}
pub fn write_frame(writer: &mut impl Write, frame: &ExtensionFrame) -> Result<(), FormatError> {
    writer
        .write_all(&frame.encode_line()?)
        .and_then(|()| writer.flush())
        .map_err(|e| invalid("ToolFailed", e.to_string()))
}
fn operation(port: &str) -> Option<&'static str> {
    match port {
        "contract-reader" => Some("read"),
        "generator" => Some("generate"),
        "toolchain" => Some("build"),
        "verifier" => Some("verify"),
        "artifact-packager" => Some("package"),
        _ => None,
    }
}
fn invalid(code: &'static str, message: impl Into<String>) -> FormatError {
    FormatError::new(code, "extension-author", message)
}
