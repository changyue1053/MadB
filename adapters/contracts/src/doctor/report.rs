use super::{invalid, selector};
use crate::project_init::wire::*;
use madb_app::use_cases::{
    doctor::*,
    initialize::{Diagnostic, Recovery, RecoveryKind, ToolObservation, VerifyLevel},
};
use madb_app::{ErrorKind, Result};
use madb_model::contract::Direction;
use serde_json::{Value, json};

fn file_value(v: &FileIdentity) -> Value {
    json!({"path":v.path.as_str(),"sha256":v.sha256.as_str(),"size":v.size})
}
fn reference_value(v: &FileReference) -> Value {
    let mut value = file_value(&v.file);
    value["schema"] = json!(v.schema.as_str());
    value
}
fn component_value(v: &ComponentIdentity) -> Value {
    json!({"id":v.id.as_str(),"version":v.version,"sha256":v.sha256.as_str()})
}
fn diagnostics_value(v: &[Diagnostic]) -> Value {
    json!(v.iter().map(|d|json!({"code":d.code.as_str(),"message":d.message,"location":d.location.as_ref().map(|p|json!({"path":p.path.as_str(),"line":p.line,"column":p.column}))})).collect::<Vec<_>>())
}
fn tool_value(v: &ToolObservation) -> Value {
    json!({"id":v.id.as_str(),"version":v.version,"sha256":v.sha256.as_str(),"host":v.host.as_str(),"target":v.target.as_str(),"source":v.source,"arguments":v.arguments})
}
fn fact_value(v: &Fact) -> Value {
    match v {
        Fact::Bytes { sha256, size } => {
            json!({"kind":"bytes","sha256":sha256.as_str(),"size":size})
        }
        Fact::Digest { domain, sha256 } => {
            json!({"kind":"digest","domain":match domain{DigestKind::Project=>"madb.project/1",DigestKind::Profile=>"madb.profile/1",DigestKind::Source=>"madb.source/1"},"sha256":sha256.as_str()})
        }
        Fact::Tool(v) => json!({"kind":"tool","method":"locked-bytes","value":tool_value(v)}),
        Fact::Host {
            os,
            arch,
            version,
            build,
        } => {
            json!({"kind":"host","os":os.as_str(),"arch":arch.as_str(),"version":version,"build":build})
        }
        Fact::State(v) => {
            json!({"kind":"state","value":match v{State::Checked=>"checked",State::Supported=>"supported",State::Missing=>"missing",State::Unsupported=>"unsupported",State::NotChecked=>"not-checked"}})
        }
    }
}
fn kind(v: CheckKind) -> &'static str {
    match v {
        CheckKind::Configuration => "configuration",
        CheckKind::Input => "input",
        CheckKind::Recipe => "recipe",
        CheckKind::Host => "host",
        CheckKind::Tool => "tool",
        CheckKind::Sdk => "sdk",
        CheckKind::Resource => "resource",
    }
}
fn error_kind(v: ErrorKind) -> &'static str {
    match v {
        ErrorKind::CheckFailed => "check-failed",
        ErrorKind::InvalidInput => "invalid-input",
        ErrorKind::Host => "host",
        ErrorKind::Conflict => "conflict",
    }
}
pub(super) fn encode(v: &Report) -> Value {
    let context=v.context.as_ref().map(|c|json!({"project":file_value(&c.project),"project_format":match c.project_format{ProjectEncoding::Json=>"json",ProjectEncoding::Toml=>"toml"},"read_request":reference_value(&c.read_request),"lock":reference_value(&c.lock),"profile":reference_value(&c.profile),"snapshot":c.snapshot.as_ref().map(reference_value),"selector":{"route":c.selector.route.as_str(),"profile_id":c.selector.profile_id.as_str(),"profile_digest":c.selector.profile_digest.as_str(),"target":c.selector.target.as_str(),"direction":match c.selector.direction{Direction::ConsumerToProvider=>"consumer-to-provider",Direction::HostCallback=>"host-callback",Direction::ReverseExport=>"reverse-export"}},"recipe":c.recipe.as_ref().map(component_value)}));
    json!({"schema":"madb.doctor-result/1","tool":v.tool.as_ref().map(component_value),"request":v.request.as_ref().map(|r|json!({"root":{"path":r.root.as_str(),"redacted":false},"project":r.project.as_str(),"read_request":r.read_request.as_str(),"snapshot":r.snapshot.as_str(),"verify":r.verify.as_str()})),"context":context,"scope":"preflight-only","truncated":v.truncated,"checks":v.checks.iter().map(|c|json!({"id":c.id.as_str(),"kind":kind(c.kind),"subject":c.subject,"status":match c.status{Status::Passed=>"passed",Status::Failed=>"failed",Status::Error=>"error",Status::Unsupported=>"unsupported",Status::Blocked=>"blocked"},"error_kind":c.error_kind.map(error_kind),"expected":c.expected.as_ref().map(fact_value),"observed":c.observed.as_ref().map(fact_value),"diagnostics":diagnostics_value(&c.diagnostics),"blocked_by":c.blocked_by.iter().map(|id|id.as_str()).collect::<Vec<_>>()})).collect::<Vec<_>>(),"recovery":v.recovery.iter().map(|r|json!({"kind":match r.kind{RecoveryKind::Retry=>"retry",RecoveryKind::Manual=>"manual",RecoveryKind::Inspect=>"inspect"},"message":r.message,"arguments":r.arguments})).collect::<Vec<_>>(),"diagnostics":diagnostics_value(&v.diagnostics),"exit_code":v.exit_code})
}
fn diagnostic_list(v: &Value) -> Result<Vec<Diagnostic>> {
    array(v)?
        .iter()
        .map(|d| {
            Ok(Diagnostic {
                code: id(&d["code"])?,
                message: string(&d["message"])?.into(),
                location: optional(&d["location"], location)?,
            })
        })
        .collect()
}
fn tool(v: &Value) -> Result<ToolObservation> {
    Ok(ToolObservation {
        id: id(&v["id"])?,
        version: string(&v["version"])?.into(),
        sha256: digest(&v["sha256"])?,
        host: id(&v["host"])?,
        target: id(&v["target"])?,
        source: string(&v["source"])?.into(),
        arguments: strings(&v["arguments"])?,
    })
}
fn fact(v: &Value) -> Result<Fact> {
    Ok(match string(&v["kind"])? {
        "bytes" => Fact::Bytes {
            sha256: digest(&v["sha256"])?,
            size: uint(&v["size"])?,
        },
        "digest" => Fact::Digest {
            domain: match string(&v["domain"])? {
                "madb.project/1" => DigestKind::Project,
                "madb.profile/1" => DigestKind::Profile,
                "madb.source/1" => DigestKind::Source,
                _ => return Err(invalid("unknown digest domain")),
            },
            sha256: digest(&v["sha256"])?,
        },
        "tool" => Fact::Tool(tool(&v["value"])?),
        "host" => Fact::Host {
            os: id(&v["os"])?,
            arch: id(&v["arch"])?,
            version: optional(&v["version"], |s| Ok(string(s)?.into()))?,
            build: optional(&v["build"], |s| Ok(string(s)?.into()))?,
        },
        "state" => Fact::State(match string(&v["value"])? {
            "checked" => State::Checked,
            "supported" => State::Supported,
            "missing" => State::Missing,
            "unsupported" => State::Unsupported,
            "not-checked" => State::NotChecked,
            _ => return Err(invalid("unknown state fact")),
        }),
        _ => return Err(invalid("unknown fact")),
    })
}
fn request(v: &Value) -> Result<Request> {
    Ok(Request {
        root: RootPath::new(string(&v["root"]["path"])?)?,
        project: path(&v["project"])?,
        read_request: path(&v["read_request"])?,
        snapshot: path(&v["snapshot"])?,
        verify: match string(&v["verify"])? {
            "structure" => VerifyLevel::Structure,
            "build" => VerifyLevel::Build,
            "run" => VerifyLevel::Run,
            _ => return Err(invalid("unknown verification level")),
        },
    })
}
fn context(v: &Value) -> Result<Context> {
    Ok(Context {
        project: file(&v["project"])?,
        project_format: match string(&v["project_format"])? {
            "json" => ProjectEncoding::Json,
            "toml" => ProjectEncoding::Toml,
            _ => return Err(invalid("unknown project format")),
        },
        read_request: reference(&v["read_request"])?,
        lock: reference(&v["lock"])?,
        profile: reference(&v["profile"])?,
        snapshot: optional(&v["snapshot"], reference)?,
        selector: selector(&v["selector"])?,
        recipe: optional(&v["recipe"], component)?,
    })
}
fn check(v: &Value) -> Result<Check> {
    Ok(Check {
        id: id(&v["id"])?,
        kind: match string(&v["kind"])? {
            "configuration" => CheckKind::Configuration,
            "input" => CheckKind::Input,
            "recipe" => CheckKind::Recipe,
            "host" => CheckKind::Host,
            "tool" => CheckKind::Tool,
            "sdk" => CheckKind::Sdk,
            "resource" => CheckKind::Resource,
            _ => return Err(invalid("unknown check kind")),
        },
        subject: string(&v["subject"])?.into(),
        status: match string(&v["status"])? {
            "passed" => Status::Passed,
            "failed" => Status::Failed,
            "error" => Status::Error,
            "unsupported" => Status::Unsupported,
            "blocked" => Status::Blocked,
            _ => return Err(invalid("unknown check status")),
        },
        error_kind: optional(&v["error_kind"], |v| {
            Ok(match string(v)? {
                "invalid-input" => ErrorKind::InvalidInput,
                "check-failed" => ErrorKind::CheckFailed,
                "host" => ErrorKind::Host,
                "conflict" => ErrorKind::Conflict,
                _ => return Err(invalid("unknown error kind")),
            })
        })?,
        expected: optional(&v["expected"], fact)?,
        observed: optional(&v["observed"], fact)?,
        diagnostics: diagnostic_list(&v["diagnostics"])?,
        blocked_by: ids(&v["blocked_by"])?,
    })
}
pub(super) fn decode(v: &Value) -> Result<Report> {
    Ok(Report {
        tool: optional(&v["tool"], component)?,
        request: optional(&v["request"], request)?,
        context: optional(&v["context"], context)?,
        truncated: boolean(&v["truncated"])?,
        checks: array(&v["checks"])?
            .iter()
            .map(check)
            .collect::<Result<_>>()?,
        recovery: array(&v["recovery"])?
            .iter()
            .map(|r| {
                Ok(Recovery {
                    kind: match string(&r["kind"])? {
                        "retry" => RecoveryKind::Retry,
                        "manual" => RecoveryKind::Manual,
                        "inspect" => RecoveryKind::Inspect,
                        _ => return Err(invalid("unknown recovery kind")),
                    },
                    message: string(&r["message"])?.into(),
                    arguments: optional(&r["arguments"], strings)?,
                })
            })
            .collect::<Result<_>>()?,
        diagnostics: diagnostic_list(&v["diagnostics"])?,
        exit_code: uint(&v["exit_code"])? as u8,
    })
}
