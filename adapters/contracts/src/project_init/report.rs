use super::{decode, invalid, wire::*};
use madb_app::use_cases::initialize::*;
use madb_app::{Digest, Id, Result};
use madb_model::evidence::{Applicability, EvidenceLevel, ExecutionStatus};
use madb_plan::project::{
    CheckLevel, ComponentIdentity, FileIdentity, FileOwnership, FileReference, IdeEntry,
    ParameterValue,
};
use madb_protocol::Value;
use serde_json::json;
use std::collections::BTreeSet;

pub(super) fn request_projection(p: &ResolvedRequest) -> Value {
    let parameters: serde_json::Map<String, Value> = p
        .parameter_values
        .iter()
        .map(|(id, v)| {
            (
                id.to_string(),
                match v {
                    ParameterValue::String(v) => json!(v),
                    ParameterValue::Boolean(v) => json!(v),
                    ParameterValue::Integer(v) => json!(v),
                },
            )
        })
        .collect();
    json!({"template_selector":p.template_selector,"template_profile_selector":p.template_profile_selector.as_ref().map(Id::as_str),"parameter_values":parameters,"targets":ids_value(&p.targets),"verify":p.verify.as_str(),"open":p.open.as_ref().map(Id::as_str),"dry_run":p.dry_run,"offline":p.offline,"output_path":p.output.as_str()})
}
fn ids_value(ids: &[Id]) -> Value {
    json!(ids.iter().map(Id::as_str).collect::<Vec<_>>())
}
fn file_value(v: &FileIdentity) -> Value {
    json!({"path":v.path.as_str(),"sha256":v.sha256.as_str(),"size":v.size})
}
fn ref_value(v: &FileReference) -> Value {
    let mut f = file_value(&v.file);
    f["schema"] = json!(v.schema.as_str());
    f
}
fn component_value(v: &ComponentIdentity) -> Value {
    json!({"id":v.id.as_str(),"version":v.version,"sha256":v.sha256.as_str()})
}
fn path_value(v: &DisplayPath, tag: &str) -> Value {
    match v {
        DisplayPath::Absolute(p) => json!({"path":p.as_str(),"redacted":false}),
        DisplayPath::Redacted => json!({"path":tag,"redacted":true}),
    }
}
fn diagnostic_value(v: &Diagnostic) -> Value {
    json!({"code":v.code.as_str(),"message":v.message,"location":v.location.as_ref().map(|l|json!({"path":l.path.as_str(),"line":l.line,"column":l.column}))})
}
fn diagnostics(v: &[Diagnostic]) -> Value {
    json!(v.iter().map(diagnostic_value).collect::<Vec<_>>())
}
fn files(v: &[FileIdentity]) -> Value {
    json!(v.iter().map(file_value).collect::<Vec<_>>())
}
fn level_value(v: CheckLevel) -> &'static str {
    match v {
        CheckLevel::Structure => "structure",
        CheckLevel::Build => "build",
        CheckLevel::Run => "run",
        CheckLevel::Import => "import",
        CheckLevel::Debug => "debug",
    }
}
fn evidence_value(v: EvidenceLevel) -> &'static str {
    match v {
        EvidenceLevel::Static => "static",
        EvidenceLevel::Generated => "generated",
        EvidenceLevel::Compiled => "compiled",
        EvidenceLevel::Linked => "linked",
        EvidenceLevel::HostRun => "host-run",
        EvidenceLevel::TargetRun => "target-run",
        EvidenceLevel::DeviceRun => "device-run",
    }
}
fn status_value(v: ExecutionStatus) -> &'static str {
    match v {
        ExecutionStatus::Passed => "passed",
        ExecutionStatus::Failed => "failed",
        ExecutionStatus::Error => "error",
        ExecutionStatus::Skipped => "skipped",
        ExecutionStatus::Unsupported => "unsupported",
        ExecutionStatus::Blocked => "blocked",
    }
}
fn ownership_value(v: FileOwnership) -> &'static str {
    match v {
        FileOwnership::User => "user",
        FileOwnership::Managed => "managed",
        FileOwnership::LocalState => "local-state",
        FileOwnership::Provenance => "provenance",
    }
}
fn ide_value(v: &IdeEntry) -> Value {
    json!({"id":v.id.as_str(),"module":v.module.as_str(),"ide":{"id":v.ide.id.as_str(),"min_inclusive":v.ide.min_inclusive,"max_exclusive":v.ide.max_exclusive},"path":v.path.as_str(),"run_target":v.run_target.as_ref().map(Id::as_str),"extensions":v.extensions.iter().map(|e|json!({"id":e.id.as_str(),"version":e.version})).collect::<Vec<_>>()})
}
fn check_value(v: &CheckObservation) -> Value {
    json!({"id":v.id.as_str(),"level":level_value(v.level),"target":v.target.as_str(),"status":status_value(v.status),"requirement":match v.requirement{Applicability::Required=>"required",Applicability::Optional=>"optional",Applicability::NotApplicable=>"not-applicable"},"environment":v.environment.iter().map(|e|json!({"id":e.id.as_str(),"version":e.version,"sha256":e.sha256.as_str(),"host":e.host.as_str(),"target":e.target.as_str(),"source":e.source,"arguments":e.arguments})).collect::<Vec<_>>(),"command":v.command,"evidence_level":v.evidence_level.map(evidence_value),"cleanup":match v.cleanup{Cleanup::Complete=>"complete",Cleanup::Failed=>"failed",Cleanup::NotRun=>"not-run"},"diagnostics":diagnostics(&v.diagnostics),"attachments":files(&v.attachments),"reason":v.reason})
}
pub(super) fn encode_report(v: &InitReport) -> Value {
    json!({"schema":"madb.init-result/1","request_digest":v.request_digest.as_ref().map(Digest::as_str),"tool":component_value(&v.tool),
        "template":v.template.as_ref().map(|t|json!({"id":t.id.as_str(),"version":t.version.to_string(),"manifest":ref_value(&t.manifest),"package_sha256":t.package_sha256.as_str(),"input_root":t.input_root.as_str()})),
        "template_package":v.template_package.as_ref().map(file_value),"evidence_root":v.evidence_root.as_ref().map(|p|path_value(p,"<evidence-root>")),"template_profile":v.template_profile.as_ref().map(Id::as_str),"boundary_profiles":v.boundary_profiles.iter().map(ref_value).collect::<Vec<_>>(),
        "request":v.request.as_ref().map(|r|json!({"targets":ids_value(&r.targets),"verify":r.verify.as_str(),"open":r.open.as_ref().map(Id::as_str),"dry_run":r.dry_run,"offline":r.offline})),"output":v.output.as_ref().map(|p|path_value(p,"<project-root>")),
        "plan":v.plan.as_ref().map(|p|json!({"files":p.files.iter().map(|f|json!({"module":f.module.as_str(),"path":f.path.as_str(),"ownership":ownership_value(f.ownership),"sha256":f.content.as_ref().map(|(d,_)|d.as_str()),"size":f.content.as_ref().map(|(_,s)|s)})).collect::<Vec<_>>(),"task_roots":ids_value(&p.task_roots),"downloads":p.downloads.iter().map(|d|json!({"id":d.id.as_str(),"version":d.version,"source":match &d.source{DownloadSource::Bundled=>json!({"kind":"bundled"}),DownloadSource::Https{url}=>json!({"kind":"https","url":url})},"sha256":d.sha256.as_str(),"size":d.size})).collect::<Vec<_>>()})),
        "creation":match v.creation{Creation::NotCreated=>"not-created",Creation::Created=>"created"},"termination":match v.termination{Termination::Completed=>"completed",Termination::Failed=>"failed",Termination::Cancelled=>"cancelled"},"exit_code":v.exit_code,"checks":v.checks.iter().map(check_value).collect::<Vec<_>>(),"ide_entries":v.ide_entries.iter().map(ide_value).collect::<Vec<_>>(),
        "open":{"requested":v.open.requested.as_ref().map(Id::as_str),"status":status_value(v.open.status),"diagnostics":diagnostics(&v.open.diagnostics),"attachments":files(&v.open.attachments)},"recovery":v.recovery.iter().map(|r|json!({"kind":match r.kind{RecoveryKind::Retry=>"retry",RecoveryKind::Manual=>"manual",RecoveryKind::Inspect=>"inspect"},"message":r.message,"arguments":r.arguments})).collect::<Vec<_>>(),"diagnostics":diagnostics(&v.diagnostics),"logs":files(&v.logs)})
}
fn status(v: &Value) -> Result<ExecutionStatus> {
    match string(v)? {
        "passed" => Ok(ExecutionStatus::Passed),
        "failed" => Ok(ExecutionStatus::Failed),
        "error" => Ok(ExecutionStatus::Error),
        "skipped" => Ok(ExecutionStatus::Skipped),
        "unsupported" => Ok(ExecutionStatus::Unsupported),
        "blocked" => Ok(ExecutionStatus::Blocked),
        _ => Err(invalid("status", "unknown check status")),
    }
}
fn verify(v: &Value) -> Result<VerifyLevel> {
    match string(v)? {
        "structure" => Ok(VerifyLevel::Structure),
        "build" => Ok(VerifyLevel::Build),
        "run" => Ok(VerifyLevel::Run),
        _ => Err(invalid("verify", "unsupported verification level")),
    }
}
fn display(v: &Value, tag: &str) -> Result<DisplayPath> {
    if boolean(&v["redacted"])? {
        if string(&v["path"])? != tag {
            return Err(invalid("path", "wrong redaction marker"));
        }
        Ok(DisplayPath::Redacted)
    } else {
        Ok(DisplayPath::Absolute(AbsolutePath::new(string(
            &v["path"],
        )?)?))
    }
}
fn diagnostic(v: &Value) -> Result<Diagnostic> {
    Ok(Diagnostic {
        code: super::diagnostic::registered_code(id(&v["code"])?)?,
        message: string(&v["message"])?.into(),
        location: optional(&v["location"], location)?,
    })
}
fn diagnostic_list(v: &Value) -> Result<Vec<Diagnostic>> {
    array(v)?.iter().map(diagnostic).collect()
}
fn file_list(v: &Value) -> Result<Vec<FileIdentity>> {
    array(v)?.iter().map(file).collect()
}
fn argv(v: &Value) -> Result<Option<Vec<String>>> {
    optional(v, |v| {
        let values = strings(v)?;
        if values.is_empty() || values[0].is_empty() {
            return Err(invalid("argv", "command must have a nonempty executable"));
        }
        Ok(values)
    })
}
fn check(v: &Value) -> Result<CheckObservation> {
    Ok(CheckObservation {
        id: id(&v["id"])?,
        level: level(&v["level"])?,
        target: id(&v["target"])?,
        status: status(&v["status"])?,
        requirement: match string(&v["requirement"])? {
            "required" => Applicability::Required,
            "optional" => Applicability::Optional,
            "not-applicable" => Applicability::NotApplicable,
            _ => return Err(invalid("requirement", "unknown requirement")),
        },
        environment: array(&v["environment"])?
            .iter()
            .map(|v| {
                Ok(ToolObservation {
                    id: id(&v["id"])?,
                    version: string(&v["version"])?.into(),
                    sha256: digest(&v["sha256"])?,
                    host: id(&v["host"])?,
                    target: id(&v["target"])?,
                    source: string(&v["source"])?.into(),
                    arguments: strings(&v["arguments"])?,
                })
            })
            .collect::<Result<_>>()?,
        command: argv(&v["command"])?,
        evidence_level: optional(&v["evidence_level"], evidence)?,
        cleanup: match string(&v["cleanup"])? {
            "complete" => Cleanup::Complete,
            "failed" => Cleanup::Failed,
            "not-run" => Cleanup::NotRun,
            _ => return Err(invalid("cleanup", "unknown cleanup")),
        },
        diagnostics: diagnostic_list(&v["diagnostics"])?,
        attachments: file_list(&v["attachments"])?,
        reason: optional(&v["reason"], |v| Ok(string(v)?.to_owned()))?,
    })
}
fn plan(v: &Value) -> Result<PlanObservation> {
    Ok(PlanObservation {
        files: array(&v["files"])?
            .iter()
            .map(|v| {
                let owner = ownership(&v["ownership"])?;
                let content = match (v["sha256"].is_null(), v["size"].is_null()) {
                    (true, true) => None,
                    (false, false) => Some((digest(&v["sha256"])?, uint(&v["size"])?)),
                    _ => return Err(invalid("plan.files", "partial content identity")),
                };
                if (owner == FileOwnership::LocalState) != content.is_none() {
                    return Err(invalid(
                        "plan.files",
                        "only local-state has no content identity",
                    ));
                }
                Ok(PlannedFileObservation {
                    module: id(&v["module"])?,
                    path: path(&v["path"])?,
                    ownership: owner,
                    content,
                })
            })
            .collect::<Result<_>>()?,
        task_roots: ids(&v["task_roots"])?,
        downloads: array(&v["downloads"])?
            .iter()
            .map(|v| {
                Ok(DownloadObservation {
                    id: id(&v["id"])?,
                    version: string(&v["version"])?.into(),
                    source: match string(&v["source"]["kind"])? {
                        "bundled" => DownloadSource::Bundled,
                        "https" => DownloadSource::Https {
                            url: string(&v["source"]["url"])?.into(),
                        },
                        _ => return Err(invalid("downloads", "unknown source")),
                    },
                    sha256: digest(&v["sha256"])?,
                    size: uint(&v["size"])?,
                })
            })
            .collect::<Result<_>>()?,
    })
}
pub(super) fn decode_report(v: &Value) -> Result<InitReport> {
    let report = InitReport {
        request_digest: optional(&v["request_digest"], digest)?,
        tool: component(&v["tool"])?,
        template: optional(&v["template"], |v| {
            Ok(TemplateObservation {
                id: id(&v["id"])?,
                version: version(&v["version"])?,
                manifest: reference(&v["manifest"])?,
                package_sha256: digest(&v["package_sha256"])?,
                input_root: path(&v["input_root"])?,
            })
        })?,
        template_package: optional(&v["template_package"], file)?,
        evidence_root: optional(&v["evidence_root"], |v| display(v, "<evidence-root>"))?,
        template_profile: optional(&v["template_profile"], id)?,
        boundary_profiles: array(&v["boundary_profiles"])?
            .iter()
            .map(reference)
            .collect::<Result<_>>()?,
        request: optional(&v["request"], |v| {
            Ok(RequestObservation {
                targets: ids(&v["targets"])?,
                verify: verify(&v["verify"])?,
                open: optional(&v["open"], id)?,
                dry_run: boolean(&v["dry_run"])?,
                offline: boolean(&v["offline"])?,
            })
        })?,
        output: optional(&v["output"], |v| display(v, "<project-root>"))?,
        plan: optional(&v["plan"], plan)?,
        creation: match string(&v["creation"])? {
            "not-created" => Creation::NotCreated,
            "created" => Creation::Created,
            _ => return Err(invalid("creation", "unknown creation observation")),
        },
        termination: match string(&v["termination"])? {
            "completed" => Termination::Completed,
            "failed" => Termination::Failed,
            "cancelled" => Termination::Cancelled,
            _ => return Err(invalid("termination", "unknown termination")),
        },
        exit_code: uint(&v["exit_code"])? as u8,
        checks: array(&v["checks"])?
            .iter()
            .map(check)
            .collect::<Result<_>>()?,
        ide_entries: array(&v["ide_entries"])?
            .iter()
            .map(decode::ide)
            .collect::<Result<_>>()?,
        open: OpenObservation {
            requested: optional(&v["open"]["requested"], id)?,
            status: status(&v["open"]["status"])?,
            diagnostics: diagnostic_list(&v["open"]["diagnostics"])?,
            attachments: file_list(&v["open"]["attachments"])?,
        },
        recovery: array(&v["recovery"])?
            .iter()
            .map(|v| {
                Ok(Recovery {
                    kind: match string(&v["kind"])? {
                        "retry" => RecoveryKind::Retry,
                        "manual" => RecoveryKind::Manual,
                        "inspect" => RecoveryKind::Inspect,
                        _ => return Err(invalid("recovery", "unknown recovery kind")),
                    },
                    message: string(&v["message"])?.into(),
                    arguments: argv(&v["arguments"])?,
                })
            })
            .collect::<Result<_>>()?,
        diagnostics: diagnostic_list(&v["diagnostics"])?,
        logs: file_list(&v["logs"])?,
    };
    check_report(&report)?;
    Ok(report)
}
fn check_report(r: &InitReport) -> Result<()> {
    if r.template.is_some() != r.template_package.is_some() {
        return Err(invalid(
            "template",
            "template and original archive reference must be jointly present",
        ));
    }
    if let (Some(t), Some(p)) = (&r.template, &r.template_package) {
        if t.package_sha256 != p.sha256 {
            return Err(invalid("package", "archive identity differs"));
        }
        let prefix = format!("{}/", t.input_root.as_str());
        if t.manifest.schema.as_str() != "madb.project-template/1"
            || !t.manifest.file.path.as_str().starts_with(&prefix)
            || r.boundary_profiles.iter().any(|p| {
                !p.file.path.as_str().starts_with(&prefix)
                    || !matches!(p.schema.as_str(), "madb.profile/1" | "madb.profile/2")
            })
        {
            return Err(invalid(
                "input_root",
                "outer references do not preserve the declared input root",
            ));
        }
    } else if !r.boundary_profiles.is_empty() {
        return Err(invalid(
            "template",
            "boundary references require template provenance",
        ));
    }
    let requested = r.request.as_ref().and_then(|q| q.open.as_ref());
    if r.open.requested.as_ref() != requested {
        return Err(invalid("open", "open request and observation differ"));
    }
    if requested.is_none() && r.open.status != ExecutionStatus::Skipped {
        return Err(invalid(
            "open",
            "unrequested IDE launch must remain skipped",
        ));
    }
    let mut keys = BTreeSet::new();
    for c in &r.checks {
        if !keys.insert((&c.id, &c.target)) {
            return Err(invalid("checks", "duplicate check key"));
        }
        if (matches!(
            c.status,
            ExecutionStatus::Skipped | ExecutionStatus::Unsupported | ExecutionStatus::Blocked
        ) || c.requirement == Applicability::NotApplicable)
            && c.reason.as_ref().is_none_or(String::is_empty)
        {
            return Err(invalid(c.id.as_str(), "unexecuted check requires a reason"));
        }
    }
    if let Some(q) = &r.request
        && (q.targets.is_empty() || !q.targets.windows(2).all(|w| w[0] < w[1]))
    {
        return Err(invalid(
            "targets",
            "resolved families must be sorted unique and nonempty",
        ));
    }
    if let Some(p) = &r.plan {
        if !p.files.windows(2).all(|w| w[0].path < w[1].path) {
            return Err(invalid(
                "plan.files",
                "initial file paths must be sorted and unique",
            ));
        }
        if p.task_roots.iter().collect::<BTreeSet<_>>().len() != p.task_roots.len()
            || p.downloads
                .iter()
                .map(|d| &d.id)
                .collect::<BTreeSet<_>>()
                .len()
                != p.downloads.len()
        {
            return Err(invalid("plan", "duplicate task root or download identity"));
        }
    }
    let has_files = r.template.is_some()
        || !r.boundary_profiles.is_empty()
        || !r.logs.is_empty()
        || !r.open.attachments.is_empty()
        || r.checks.iter().any(|c| !c.attachments.is_empty());
    if has_files && r.evidence_root.is_none() {
        return Err(invalid(
            "evidence_root",
            "file references require an explicit evidence root",
        ));
    }
    if r.exit_code == 0 {
        if r.termination != Termination::Completed
            || r.request_digest.is_none()
            || r.template.is_none()
            || r.template_profile.is_none()
            || r.evidence_root.is_none()
            || r.output.is_none()
            || r.plan.is_none()
        {
            return Err(invalid("success", "success observations are incomplete"));
        }
        let q = r
            .request
            .as_ref()
            .ok_or_else(|| invalid("request", "success requires request observations"))?;
        if q.dry_run {
            if r.creation != Creation::NotCreated
                || !r.checks.is_empty()
                || r.open.status != ExecutionStatus::Skipped
                || !r.plan.as_ref().expect("checked plan").downloads.is_empty()
            {
                return Err(invalid("dry-run", "dry-run records execution or creation"));
            }
        } else if r.creation != Creation::Created
            || r.checks.is_empty()
            || r.checks
                .iter()
                .filter(|c| c.requirement == Applicability::Required)
                .any(|c| c.status != ExecutionStatus::Passed)
        {
            return Err(invalid(
                "success",
                "ordinary success observations contradict creation or required checks",
            ));
        }
    } else if r.termination == Termination::Completed
        || (r.termination == Termination::Cancelled && r.exit_code != 1)
    {
        return Err(invalid(
            "termination",
            "termination contradicts the exit code",
        ));
    }
    Ok(())
}
