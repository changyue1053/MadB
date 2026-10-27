use super::invalid;
use crate::project_init::wire::{
    array, boolean, digest, file, id, optional, path, reference, string, strings, uint,
};
use madb_app::Result;
use madb_app::use_cases::project_build::*;
use madb_model::Location;
use madb_protocol::Value;

pub(super) fn tool(v: &Value) -> Result<ToolIdentity> {
    Ok(ToolIdentity {
        id: id(&v["id"])?,
        version: string(&v["version"])?.into(),
        sha256: digest(&v["sha256"])?,
        size: uint(&v["size"])?,
        source_revision: string(&v["source_revision"])?.into(),
    })
}
pub(super) fn descriptor(v: &Value) -> Result<Descriptor> {
    Ok(Descriptor {
        template: TemplateIdentity {
            id: id(&v["template"]["id"])?,
            version: string(&v["template"]["version"])?.into(),
            archive: file(&v["template"]["archive"])?,
        },
        namespace: string(&v["namespace"])?.into(),
        profile: reference(&v["profile"])?,
        sdk: reference(&v["sdk"])?,
        tool: tool(&v["tool"])?,
    })
}
fn root(v: &Value) -> Result<AbsolutePath> {
    AbsolutePath::new(string(&v["path"])?)
}
fn operation(v: &Value) -> Result<Operation> {
    match string(v)? {
        "build" => Ok(Operation::Build),
        "install-tool" => Ok(Operation::InstallTool),
        _ => Err(invalid("operation", "unknown project operation")),
    }
}
fn stage_id(v: &Value) -> Result<StageId> {
    match string(v)? {
        "capture" => Ok(StageId::Capture),
        "preflight" => Ok(StageId::Preflight),
        "prepare" => Ok(StageId::Prepare),
        "metadata" => Ok(StageId::Metadata),
        "generate" => Ok(StageId::Generate),
        "build" => Ok(StageId::Build),
        "package" => Ok(StageId::Package),
        "consumer-link" => Ok(StageId::ConsumerLink),
        "publish" => Ok(StageId::Publish),
        "install" => Ok(StageId::Install),
        _ => Err(invalid("stage", "unknown finite project stage")),
    }
}
fn status(v: &Value) -> Result<StageStatus> {
    match string(v)? {
        "passed" => Ok(StageStatus::Passed),
        "failed" => Ok(StageStatus::Failed),
        "error" => Ok(StageStatus::Error),
        "blocked" => Ok(StageStatus::Blocked),
        "skipped" => Ok(StageStatus::Skipped),
        "unsupported" => Ok(StageStatus::Unsupported),
        _ => Err(invalid("status", "unknown project status")),
    }
}
fn diagnostic(v: &Value) -> Result<Diagnostic> {
    Ok(Diagnostic {
        code: id(&v["code"])?,
        message: string(&v["message"])?.into(),
        location: optional(&v["location"], |l| {
            Ok(Location {
                path: path(&l["path"])?,
                line: uint(&l["line"])?,
                column: uint(&l["column"])?,
            })
        })?,
    })
}
fn diagnostics(v: &Value) -> Result<Vec<Diagnostic>> {
    array(v)?.iter().map(diagnostic).collect()
}
fn files(v: &Value) -> Result<Vec<FileIdentity>> {
    array(v)?.iter().map(file).collect()
}
pub(super) fn inputs(v: &Value) -> Result<InputInventory> {
    Ok(InputInventory {
        files: array(&v["files"])?
            .iter()
            .map(|f| {
                Ok(InputFile {
                    source: path(&f["source"])?,
                    file: file(&f["file"])?,
                })
            })
            .collect::<Result<_>>()?,
        directories: array(&v["directories"])?
            .iter()
            .map(|d| {
                Ok(InputDirectory {
                    path: path(&d["path"])?,
                    present: boolean(&d["present"])?,
                    members: array(&d["members"])?
                        .iter()
                        .map(|m| {
                            Ok(DirectoryMember {
                                path: path(&m["path"])?,
                                kind: match string(&m["kind"])? {
                                    "file" => MemberKind::File,
                                    "directory" => MemberKind::Directory,
                                    _ => {
                                        return Err(invalid(
                                            "kind",
                                            "unknown directory member kind",
                                        ));
                                    }
                                },
                            })
                        })
                        .collect::<Result<_>>()?,
                })
            })
            .collect::<Result<_>>()?,
    })
}
fn output(v: &Value) -> Result<ProjectOutput> {
    Ok(ProjectOutput {
        identity: digest(&v["identity"])?,
        projection: OutputProjection {
            snapshot_digest: digest(&v["snapshot_digest"])?,
            candidate: CandidateOutput {
                root: path(&v["candidate"]["root"])?,
                manifest: file(&v["candidate"]["manifest"])?,
            },
            consumer: file(&v["consumer"])?,
            outputs: files(&v["outputs"])?,
        },
    })
}
pub(super) fn report(v: &Value) -> Result<ProjectReport> {
    Ok(ProjectReport {
        operation: operation(&v["operation"])?,
        root: optional(&v["root"], root)?,
        recipe: optional(&v["recipe"], id)?,
        request_digest: optional(&v["request_digest"], digest)?,
        tool: optional(&v["tool"], tool)?,
        artifact_root: optional(&v["artifact_root"], root)?,
        inputs: inputs(&v["inputs"])?,
        stages: array(&v["stages"])?
            .iter()
            .map(|s| {
                Ok(StageObservation {
                    id: stage_id(&s["id"])?,
                    status: status(&s["status"])?,
                    diagnostics: diagnostics(&s["diagnostics"])?,
                    attachments: files(&s["attachments"])?,
                    blocked_by: array(&s["blocked_by"])?
                        .iter()
                        .map(stage_id)
                        .collect::<Result<_>>()?,
                })
            })
            .collect::<Result<_>>()?,
        result: optional(&v["result"], output)?,
        installed_tool: optional(&v["installed_tool"], |t| {
            Ok(InstalledTool {
                path: root(&t["path"])?,
                sha256: digest(&t["sha256"])?,
                size: uint(&t["size"])?,
            })
        })?,
        previous: optional(&v["previous"], |p| {
            Ok(PreviousBuild {
                artifact_root: root(&p["artifact_root"])?,
                identity: digest(&p["identity"])?,
            })
        })?,
        published: optional(&v["published"], boolean)?,
        publication_durability: match string(&v["publication_durability"])? {
            "not-applicable" => Durability::NotApplicable,
            "confirmed" => Durability::Confirmed,
            "uncertain" => Durability::Uncertain,
            _ => return Err(invalid("durability", "unknown publication durability")),
        },
        termination: match string(&v["termination"])? {
            "complete" => Termination::Complete,
            "failed" => Termination::Failed,
            "cancelled" => Termination::Cancelled,
            _ => return Err(invalid("termination", "unknown project termination")),
        },
        exit_code: uint(&v["exit_code"])?
            .try_into()
            .map_err(|_| invalid("exit_code", "exit code overflow"))?,
        evidence_root: optional(&v["evidence_root"], root)?,
        logs: files(&v["logs"])?,
        diagnostics: diagnostics(&v["diagnostics"])?,
        recovery: array(&v["recovery"])?
            .iter()
            .map(|r| {
                Ok(Recovery {
                    kind: string(&r["kind"])?.into(),
                    message: string(&r["message"])?.into(),
                    command: optional(&r["arguments"], strings)?,
                })
            })
            .collect::<Result<_>>()?,
    })
}
