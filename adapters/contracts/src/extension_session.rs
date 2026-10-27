//! Frozen, single-request v1 host session. This adapter owns wire validation;
//! the process host retains IO/cleanup and the application retains admission.
use crate::{
    extension::{ExtensionManifest, require_component_features},
    extension_files::BoundExtensionFiles,
};
use madb_app::{
    extension::{SessionDriver, SessionStep},
    ports::FileReference,
    process::{ProcessError, ProcessRequest, ProcessResult},
};
use madb_protocol::{ExtensionFrame, Value};
use serde_json::json;
use std::{collections::BTreeMap, time::Duration};

pub struct SessionOptions<'a> {
    pub component_id: &'a str,
    pub session_id: &'a str,
    pub request_id: &'a str,
    /// Frozen execution offers exactly the version selected by the lock.
    pub protocol: &'a str,
    pub ir: &'a str,
    pub required_features: &'a [&'a str],
    pub handshake_timeout: Duration,
    pub task_timeout: Duration,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum State {
    New,
    AwaitAck,
    Running,
    Cancelling,
    Finished,
    Broken,
}

type OutputObserver<'a> = dyn FnMut(&Value) -> ProcessResult<()> + 'a;

/// Exact execution identity from an already checked C04 artifact recipe. Its
/// stdin and cwd are deliberately excluded: E02 relocates payload bytes into
/// the fixed input domain and uses stdio for the interactive control dialogue.
struct RecipeProcess {
    tool: FileReference,
    arguments: Vec<String>,
    environment: BTreeMap<String, String>,
}
impl RecipeProcess {
    fn from_request(request: &ProcessRequest) -> Self {
        Self {
            tool: request.tool.clone(),
            arguments: request.arguments.clone(),
            environment: request.environment.clone(),
        }
    }
    fn matches(&self, actual: &ProcessRequest) -> bool {
        self.tool.path == actual.tool.path
            && self.tool.digest == actual.tool.digest
            && self.tool.size == actual.tool.size
            && self.arguments == actual.arguments
            && self.environment == actual.environment
    }
}

pub struct HostSession<'a> {
    hello: Value,
    invoke: Value,
    supported: Vec<String>,
    entrypoint: Value,
    allowed_environment: Vec<String>,
    recipe_process: Option<RecipeProcess>,
    state: State,
    cancelled: bool,
    result: Option<Value>,
    peer_diagnostic: Option<Value>,
    handshake_timeout: Duration,
    task_timeout: Duration,
    on_output: Box<OutputObserver<'a>>,
}
impl<'a> HostSession<'a> {
    pub fn new(
        manifest: &ExtensionManifest,
        files: &BoundExtensionFiles,
        options: SessionOptions<'_>,
        on_output: impl FnMut(&Value) -> ProcessResult<()> + 'a,
    ) -> ProcessResult<Self> {
        if files.composition_recipe().is_some() {
            return Err(fail(
                "CapabilityUnsupported",
                "legacy session requires the explicit composition constructor",
            ));
        }
        Self::checked(manifest, files, options, on_output, false)
    }
    /// New composition semantics are only reachable through a checked /2 recipe.
    pub fn from_composition(
        files: &BoundExtensionFiles,
        options: SessionOptions<'_>,
        on_output: impl FnMut(&Value) -> ProcessResult<()> + 'a,
    ) -> ProcessResult<Self> {
        let recipe = files
            .composition_recipe()
            .ok_or_else(|| fail("InputInvalid", "missing checked composition recipe"))?;
        let pins = recipe.pins();
        if options.component_id != pins.component_id()
            || options.required_features.to_vec()
                != pins
                    .required_features()
                    .iter()
                    .map(String::as_str)
                    .collect::<Vec<_>>()
        {
            return Err(fail(
                "ExtensionIdentityMismatch",
                "session component or mandatory semantics differ from checked pins",
            ));
        }
        Self::checked(pins.manifest(), files, options, on_output, true)
    }
    fn checked(
        manifest: &ExtensionManifest,
        files: &BoundExtensionFiles,
        options: SessionOptions<'_>,
        on_output: impl FnMut(&Value) -> ProcessResult<()> + 'a,
        composition: bool,
    ) -> ProcessResult<Self> {
        if manifest.value()["example"] == true {
            return Err(fail("InputInvalid", "design manifest cannot execute"));
        }
        if !composition {
            require_component_features(manifest, options.component_id, options.required_features)
                .map_err(format_error)?;
        }
        let component = manifest.value()["components"]
            .as_array()
            .expect("checked components")
            .iter()
            .find(|c| c["id"] == options.component_id)
            .expect("required features checked component");
        let operation = match component["port"].as_str().expect("checked port") {
            "contract-reader" => "read",
            "generator" => "generate",
            "toolchain" => "build",
            "verifier" => "verify",
            "artifact-packager" => "package",
            _ => return Err(fail("CapabilityUnsupported", "unsupported component port")),
        };
        if operation != files.operation() {
            return Err(fail(
                "CapabilityUnsupported",
                "component cannot execute this operation",
            ));
        }
        // Read/generate payloads already name their exact public component.
        // A multi-component executable must not reinterpret that request as
        // another component merely because both share an executable image.
        if let Some(expected) = files.request().value().get("component")
            && (expected["id"] != format!("{}/{}", manifest.id(), options.component_id)
                || expected["version"] != manifest.version())
        {
            return Err(fail(
                "ExtensionIdentityMismatch",
                "payload component identity differs from the selected session component",
            ));
        }
        if component["entrypoint"]["kind"] != "process" {
            return Err(fail(
                "InputInvalid",
                "component is not a process entrypoint",
            ));
        }
        for (key, version) in [("protocol", options.protocol), ("ir", options.ir)] {
            if !manifest.value()["compatibility"][key]
                .as_array()
                .expect("checked versions")
                .iter()
                .any(|v| v == version)
            {
                return Err(fail(
                    "ProtocolIncompatible",
                    "locked version is absent from the manifest",
                ));
            }
        }
        for timeout in [options.handshake_timeout, options.task_timeout] {
            if timeout.is_zero() || timeout > Duration::from_secs(3600) {
                return Err(fail("InputInvalid", "invalid session timeout"));
            }
        }
        let hello = json!({"schema":"madb.extension-wire/1", "kind":"hello", "session_id":options.session_id,
            "extension_id":manifest.id(), "extension_version":manifest.version(), "component_id":options.component_id,
            "manifest_digest":manifest.manifest_digest(), "protocol_versions":[options.protocol], "ir_versions":[options.ir], "required_features":options.required_features});
        let invoke = json!({"schema":"madb.extension-wire/1", "kind":"invoke", "session_id":options.session_id,
            "request_id":options.request_id, "protocol":options.protocol, "ir":options.ir,
            "component":format!("{}/{}", manifest.id(), options.component_id), "operation":operation,
            "input":files.input_ref(), "output_root":files.output_root(), "timeout_ms":options.task_timeout.as_millis() as u64});
        ExtensionFrame::from_value(hello.clone()).map_err(format_error)?;
        ExtensionFrame::from_value(invoke.clone()).map_err(format_error)?;
        Ok(Self {
            hello,
            invoke,
            supported: component["supported_features"]
                .as_array()
                .expect("checked features")
                .iter()
                .map(|v| v.as_str().expect("feature").to_owned())
                .collect(),
            entrypoint: if composition {
                let process = files
                    .artifact_process()
                    .expect("checked composition process");
                json!({"path":process.tool.path.as_str(), "args":process.arguments})
            } else {
                component["entrypoint"].clone()
            },
            allowed_environment: manifest.value()["execution"]["env"]
                .as_array()
                .expect("checked env")
                .iter()
                .map(|v| v.as_str().expect("env").to_owned())
                .collect(),
            recipe_process: files.artifact_process().map(RecipeProcess::from_request),
            state: State::New,
            cancelled: false,
            result: None,
            peer_diagnostic: None,
            handshake_timeout: options.handshake_timeout,
            task_timeout: options.task_timeout,
            on_output: Box::new(on_output),
        })
    }
    /// Call only inside execute_process_validated after clean process exit.
    /// Recheck the same snapshot retained by on_output at the terminal event;
    /// a fresh post-exit capture must not replace that observation.
    pub fn result_reference(&self) -> ProcessResult<&Value> {
        if self.state != State::Finished || self.cancelled {
            return Err(fail("ProtocolInvalid", "no eligible terminal result"));
        }
        let result = self.result.as_ref().expect("finished result");
        match result["status"].as_str().expect("checked status") {
            "ok" => Ok(&result["output"]),
            "error" => Err(peer_error(&result["error"])),
            _ => Err(fail(
                "ToolFailed",
                "helper completed without a usable result",
            )),
        }
    }

    /// Preserve the complete schema-checked helper diagnostic, including its
    /// original code, stage and optional source, for bounded reporting.
    pub fn peer_diagnostic(&self) -> Option<&Value> {
        self.peer_diagnostic.as_ref()
    }
    fn receive_checked(&mut self, frame: Value) -> ProcessResult<SessionStep> {
        if frame["session_id"] != self.hello["session_id"] {
            return Err(fail("ProtocolInvalid", "wrong session identity"));
        }
        match self.state {
            State::AwaitAck => {
                if frame["kind"] == "reject" {
                    self.peer_diagnostic = Some(frame["error"].clone());
                    return Err(peer_error(&frame["error"]));
                }
                if frame["kind"] != "hello_ack" {
                    return Err(fail(
                        "ProtocolInvalid",
                        "expected handshake response before invocation",
                    ));
                }
                for key in [
                    "extension_id",
                    "extension_version",
                    "component_id",
                    "manifest_digest",
                ] {
                    if frame[key] != self.hello[key] {
                        return Err(fail(
                            "ExtensionIdentityMismatch",
                            "helper identity differs from the locked component",
                        ));
                    }
                }
                if frame["protocol"] != self.invoke["protocol"] || frame["ir"] != self.invoke["ir"]
                {
                    return Err(fail(
                        "ProtocolIncompatible",
                        "helper selected an unoffered frozen version",
                    ));
                }
                let features = frame["features"].as_array().expect("checked features");
                if features
                    .iter()
                    .any(|f| !self.supported.iter().any(|s| f == s))
                    || self.hello["required_features"]
                        .as_array()
                        .expect("checked required")
                        .iter()
                        .any(|f| !features.contains(f))
                {
                    return Err(fail(
                        "CapabilityUnsupported",
                        "helper features exceed its component or omit required semantics",
                    ));
                }
                self.state = State::Running;
                Ok(SessionStep::Send(encode(self.invoke.clone())?))
            }
            State::Running | State::Cancelling => {
                if frame["kind"] != "result" {
                    return Err(fail(
                        "ProtocolInvalid",
                        "expected one result for the active request",
                    ));
                }
                for key in ["request_id", "protocol", "ir"] {
                    if frame[key] != self.invoke[key] {
                        return Err(fail(
                            "ProtocolInvalid",
                            "result identity differs from the active request",
                        ));
                    }
                }
                // Freeze an observation at the terminal boundary, but never
                // return it to downstream code here. The application must
                // recheck these same bytes after EOF/exit/cleanup in its gate.
                if frame["status"] == "ok" && !self.cancelled {
                    (self.on_output)(&frame["output"])?;
                }
                if frame["status"] == "error" {
                    self.peer_diagnostic = Some(frame["error"].clone());
                }
                self.result = Some(frame);
                self.state = State::Finished;
                Ok(SessionStep::Terminal)
            }
            _ => Err(fail(
                "ProtocolInvalid",
                "unexpected frame or duplicate terminal result",
            )),
        }
    }
}
impl SessionDriver for HostSession<'_> {
    fn validate_request(&self, request: &madb_app::process::ProcessRequest) -> ProcessResult<()> {
        if self
            .recipe_process
            .as_ref()
            .is_some_and(|bound| !bound.matches(request))
        {
            return Err(fail(
                "ExtensionIdentityMismatch",
                "process bytes, arguments or environment differ from the checked artifact recipe",
            ));
        }
        if request.tool.path.as_str() != self.entrypoint["path"].as_str().expect("checked path")
            || serde_json::to_value(&request.arguments).expect("string args")
                != self.entrypoint["args"]
            || request
                .environment
                .keys()
                .any(|name| !self.allowed_environment.contains(name))
        {
            return Err(fail(
                "ExtensionIdentityMismatch",
                "process recipe differs from the selected component or its environment request",
            ));
        }
        Ok(())
    }
    fn initial(&mut self) -> ProcessResult<Vec<u8>> {
        if self.state != State::New {
            return Err(fail("ProtocolInvalid", "session driver cannot be reused"));
        }
        self.state = State::AwaitAck;
        encode(self.hello.clone())
    }
    fn receive(&mut self, line: &[u8]) -> ProcessResult<SessionStep> {
        let result = ExtensionFrame::decode_line(line)
            .map_err(format_error)
            .and_then(|frame| self.receive_checked(frame.value().clone()));
        if result.is_err() {
            self.state = State::Broken;
        }
        result
    }
    fn cancel(&mut self) -> ProcessResult<Option<Vec<u8>>> {
        self.cancelled = true;
        match self.state {
            State::Running => {
                self.state = State::Cancelling;
                Ok(Some(encode(
                    json!({"schema":"madb.extension-wire/1", "kind":"cancel", "session_id":self.invoke["session_id"], "request_id":self.invoke["request_id"], "protocol":self.invoke["protocol"], "ir":self.invoke["ir"]}),
                )?))
            }
            State::New | State::AwaitAck => {
                self.state = State::Broken;
                Ok(None)
            }
            _ => Ok(None),
        }
    }
    fn is_terminal(&self) -> bool {
        self.state == State::Finished
    }
    fn timeout(&self) -> Duration {
        if matches!(self.state, State::New | State::AwaitAck) {
            self.handshake_timeout
        } else {
            self.task_timeout
        }
    }
}
fn encode(value: Value) -> ProcessResult<Vec<u8>> {
    ExtensionFrame::from_value(value)
        .and_then(|f| f.encode_line())
        .map_err(format_error)
}
fn fail(code: &'static str, message: impl Into<String>) -> ProcessError {
    ProcessError::new(code, "extension-session", message)
}
fn format_error(e: madb_protocol::FormatError) -> ProcessError {
    fail(e.code, e.to_string())
}
fn peer_error(value: &Value) -> ProcessError {
    let code = match value["code"].as_str().expect("checked error code") {
        "ExtensionIdentityMismatch" => "ExtensionIdentityMismatch",
        "ProtocolIncompatible" => "ProtocolIncompatible",
        "CapabilityUnsupported" => "CapabilityUnsupported",
        "ProtocolInvalid" => "ProtocolInvalid",
        "InputInvalid" => "InputInvalid",
        "OutputInvalid" => "OutputInvalid",
        "ToolTimeout" => "ToolTimeout",
        _ => "ToolFailed",
    };
    fail(
        code,
        format!(
            "{} at {}: {}",
            value["code"], value["stage"], value["message"]
        ),
    )
}
