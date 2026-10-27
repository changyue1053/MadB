use madb_app::use_cases::project_build::*;
use madb_protocol::Value;
use serde_json::json;

pub(super) fn file(v: &FileIdentity) -> Value {
    json!({"path":v.path.as_str(),"sha256":v.sha256.as_str(),"size":v.size})
}
fn reference(v: &FileReference) -> Value {
    let mut r = file(&v.file);
    r["schema"] = json!(v.schema.as_str());
    r
}
pub(super) fn tool(v: &ToolIdentity) -> Value {
    json!({"id":v.id.as_str(),"version":v.version,"sha256":v.sha256.as_str(),"size":v.size,"source_revision":v.source_revision})
}
pub(super) fn descriptor(v: &Descriptor) -> Value {
    json!({
        "schema":"madb.project-build/1","recipe":RECIPE,
        "template":{"id":v.template.id.as_str(),"version":v.template.version,"archive":file(&v.template.archive)},
        "namespace":v.namespace,"profile":reference(&v.profile),"sdk":reference(&v.sdk),"tool":tool(&v.tool),
        "author":{"language":"rust","entry":"rust/src/lib.rs"},"consumer":{"language":"c","entry":"consumer/main.c","headers":"consumer/include"},"configuration":"debug"
    })
}
pub(super) fn request(v: &RequestIdentity) -> Value {
    json!({"operation":v.operation.as_str(),"root":v.root.as_str(),"frozen":true,"descriptor":file(&v.descriptor)})
}
fn utf16(v: &str) -> Vec<u16> {
    v.encode_utf16().collect()
}
pub(super) fn directory(v: &InputDirectory) -> Value {
    let mut members = v.members.iter().collect::<Vec<_>>();
    members.sort_by_key(|x| utf16(x.path.as_str()));
    json!({"path":v.path.as_str(),"present":v.present,"members":members.iter().map(|m|json!({"path":m.path.as_str(),"kind":m.kind.as_str()})).collect::<Vec<_>>()})
}
pub(super) fn inventory(v: &InputInventory) -> Value {
    json!({"files":v.files.iter().map(|f|json!({"source":f.source.as_str(),"file":file(&f.file)})).collect::<Vec<_>>(),"directories":v.directories.iter().map(directory).collect::<Vec<_>>()})
}
pub(super) fn input_projection(descriptor: &FileIdentity, v: &InputInventory) -> Value {
    let mut files = v.files.iter().collect::<Vec<_>>();
    files.sort_by_key(|x| utf16(x.source.as_str()));
    let mut directories = v.directories.iter().collect::<Vec<_>>();
    directories.sort_by_key(|x| utf16(x.path.as_str()));
    json!({"descriptor":file(descriptor),"files":files.iter().map(|f|json!({"source":f.source.as_str(),"sha256":f.file.sha256.as_str(),"size":f.file.size})).collect::<Vec<_>>(),"directories":directories.into_iter().map(directory).collect::<Vec<_>>()})
}
pub(super) fn candidate(v: &CandidateOutput) -> Value {
    json!({"root":v.root.as_str(),"manifest":file(&v.manifest)})
}
pub(super) fn output_projection(tool_identity: &ToolIdentity, v: &OutputProjection) -> Value {
    let mut outputs = v.outputs.iter().collect::<Vec<_>>();
    outputs.sort_by_key(|f| utf16(f.path.as_str()));
    json!({"recipe":RECIPE,"tool":tool(tool_identity),"snapshot_digest":v.snapshot_digest.as_str(),"candidate":candidate(&v.candidate),"consumer":file(&v.consumer),"outputs":outputs.into_iter().map(file).collect::<Vec<_>>()})
}
pub(super) fn output(v: &ProjectOutput) -> Value {
    json!({"identity":v.identity.as_str(),"snapshot_digest":v.projection.snapshot_digest.as_str(),"candidate":candidate(&v.projection.candidate),"consumer":file(&v.projection.consumer),"outputs":v.projection.outputs.iter().map(file).collect::<Vec<_>>()})
}
fn root(v: &AbsolutePath) -> Value {
    json!({"path":v.as_str(),"redacted":false})
}
fn diagnostic(v: &Diagnostic) -> Value {
    json!({"code":v.code.as_str(),"message":v.message,"location":v.location.as_ref().map(|l|json!({"path":l.path.as_str(),"line":l.line,"column":l.column}))})
}
fn stage(v: &StageObservation) -> Value {
    json!({"id":v.id.as_str(),"status":v.status.as_str(),"diagnostics":v.diagnostics.iter().map(diagnostic).collect::<Vec<_>>(),"attachments":v.attachments.iter().map(file).collect::<Vec<_>>(),"blocked_by":v.blocked_by.iter().map(|s|s.as_str()).collect::<Vec<_>>()})
}
pub(super) fn report(v: &ProjectReport) -> Value {
    json!({
        "schema":"madb.project-build-result/1","operation":v.operation.as_str(),"root":v.root.as_ref().map(root),"recipe":v.recipe.as_ref().map(|x|x.as_str()),"request_digest":v.request_digest.as_ref().map(|x|x.as_str()),"tool":v.tool.as_ref().map(tool),
        "artifact_root":v.artifact_root.as_ref().map(root),"inputs":inventory(&v.inputs),"stages":v.stages.iter().map(stage).collect::<Vec<_>>(),"result":v.result.as_ref().map(output),
        "installed_tool":v.installed_tool.as_ref().map(|t|json!({"path":root(&t.path),"sha256":t.sha256.as_str(),"size":t.size})),
        "previous":v.previous.as_ref().map(|p|json!({"artifact_root":root(&p.artifact_root),"identity":p.identity.as_str()})),"published":v.published,"publication_durability":v.publication_durability.as_str(),
        "termination":v.termination.as_str(),"exit_code":v.exit_code,"evidence_root":v.evidence_root.as_ref().map(root),"logs":v.logs.iter().map(file).collect::<Vec<_>>(),"diagnostics":v.diagnostics.iter().map(diagnostic).collect::<Vec<_>>(),
        "recovery":v.recovery.iter().map(|r|json!({"kind":r.kind,"message":r.message,"arguments":r.command})).collect::<Vec<_>>()
    })
}
