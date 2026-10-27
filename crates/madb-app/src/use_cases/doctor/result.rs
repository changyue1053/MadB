use super::*;
use crate::use_cases::initialize::{Diagnostic, Recovery, RecoveryKind, VerifyLevel};
use crate::{ErrorKind, Id, QueryError, Result};
use std::collections::BTreeSet;

pub const RESULT_LIMIT: usize = 1024 * 1024;
pub const TOOL_NAMES: [&str; 12] = [
    "ar",
    "bindgen",
    "cbindgen",
    "clang",
    "cmake",
    "finalizer",
    "libclang",
    "ninja",
    "nm",
    "python",
    "rustc",
    "values_reader",
];
pub fn error_exit(kind: ErrorKind) -> u8 {
    match kind {
        ErrorKind::CheckFailed => 1,
        ErrorKind::InvalidInput => 2,
        ErrorKind::Host => 3,
        ErrorKind::Conflict => 4,
    }
}
pub fn bounded(value: &str, max: usize) -> String {
    let mut chars = value.chars().map(|c| if c.is_control() { ' ' } else { c });
    let mut text: String = chars.by_ref().take(max).collect();
    if chars.next().is_some() {
        text = text.chars().take(max.saturating_sub(3)).collect();
        text.push_str("...");
    }
    if text.is_empty() {
        text.push('-');
    }
    text
}
pub fn diagnostics(error: &QueryError) -> Vec<Diagnostic> {
    error
        .findings
        .iter()
        .take(16)
        .map(|f| Diagnostic {
            code: Id::new(f.code).unwrap_or_else(|_| Id::new("PLAN001").expect("fixed id")),
            message: bounded(&format!("{}: {}", f.subject, f.message), 4096),
            location: f.locations.first().cloned(),
        })
        .collect()
}
impl Check {
    pub fn passed(
        id: &str,
        kind: CheckKind,
        subject: impl Into<String>,
        expected: Option<Fact>,
        observed: Fact,
    ) -> Self {
        Self {
            id: Id::new(id).expect("internal check id"),
            kind,
            subject: bounded(&subject.into(), 4096),
            status: Status::Passed,
            error_kind: None,
            expected,
            observed: Some(observed),
            diagnostics: vec![],
            blocked_by: vec![],
        }
    }
    pub fn failed(
        id: &str,
        kind: CheckKind,
        subject: impl Into<String>,
        error: &QueryError,
    ) -> Self {
        Self {
            id: Id::new(id).expect("internal check id"),
            kind,
            subject: bounded(&subject.into(), 4096),
            status: match error.kind {
                ErrorKind::Host | ErrorKind::Conflict => Status::Error,
                _ => Status::Failed,
            },
            error_kind: Some(error.kind),
            expected: None,
            observed: None,
            diagnostics: diagnostics(error),
            blocked_by: vec![],
        }
    }
    pub fn blocked(id: &str, kind: CheckKind, by: &str) -> Self {
        Self {
            id: Id::new(id).expect("internal check id"),
            kind,
            subject: id.into(),
            status: Status::Blocked,
            error_kind: None,
            expected: None,
            observed: None,
            diagnostics: vec![],
            blocked_by: vec![Id::new(by).expect("internal dependency id")],
        }
    }
}
pub fn required_checks(level: VerifyLevel) -> Vec<String> {
    let mut ids = [
        "context.configuration",
        "context.declared-inputs",
        "context.snapshot",
        "recipe",
        "tool.madb",
        "host",
    ]
    .map(str::to_owned)
    .to_vec();
    if level != VerifyLevel::Structure {
        ids.extend(TOOL_NAMES.map(|name| format!("tool.{name}")));
        ids.extend(
            [
                "sdk",
                "resource.installation",
                "resource.python",
                "resource.compiler",
                "tool.madb-source",
            ]
            .map(str::to_owned),
        );
    }
    if level == VerifyLevel::Run {
        ids.push("run-host".into());
    }
    ids.sort();
    ids
}
impl Report {
    pub fn record(&mut self, check: Check) {
        if let Some(kind) = check.error_kind {
            self.exit_code = self.exit_code.max(error_exit(kind));
        }
        if let Some(index) = self.checks.iter().position(|v| v.id == check.id) {
            self.checks[index] = check;
        } else {
            self.checks.push(check);
        }
    }
    pub fn fail(&mut self, error: &QueryError) {
        let current = self
            .checks
            .iter()
            .find(|c| c.id.as_str() == "context.configuration")
            .and_then(|c| c.error_kind);
        if current.is_none_or(|kind| error_exit(kind) < error_exit(error.kind)) {
            self.record(Check::failed(
                "context.configuration",
                CheckKind::Configuration,
                "doctor query",
                error,
            ));
        }
        self.diagnostics.extend(
            diagnostics(error)
                .into_iter()
                .take(128 - self.diagnostics.len().min(128)),
        );
    }
    pub fn recovery(&mut self) {
        if self.exit_code != 0 && self.recovery.is_empty() {
            self.recovery.push(Recovery{kind:RecoveryKind::Inspect,message:"Inspect the reported frozen inputs and restore the exact locked installation. Prepare matching metadata explicitly when required; doctor performs no repair, installation or build. Do not rerun init over this existing project.".into(),arguments:None});
        }
    }
    /// Only a complete independent built-in set can produce exit zero.
    pub fn finish(&mut self) -> Result<()> {
        self.order()?;
        self.exit_code = self
            .checks
            .iter()
            .filter_map(|c| c.error_kind)
            .map(error_exit)
            .max()
            .unwrap_or(0)
            .max(if self.truncated { 3 } else { 0 });
        self.recovery();
        self.validate()?;
        Ok(())
    }
    fn order(&mut self) -> Result<()> {
        let mut remaining = self.checks.clone();
        let mut sorted = Vec::new();
        let mut seen = BTreeSet::new();
        while !remaining.is_empty() {
            let next = remaining
                .iter()
                .enumerate()
                .filter(|(_, v)| self.dependencies(v).iter().all(|id| seen.contains(id)))
                .min_by_key(|(_, v)| &v.id)
                .map(|(index, _)| index)
                .ok_or_else(|| {
                    QueryError::new(
                        ErrorKind::InvalidInput,
                        "PLAN001",
                        "doctor result",
                        "blocked graph contains a cycle or missing dependency",
                    )
                })?;
            let check = remaining.remove(next);
            seen.insert(check.id.clone());
            sorted.push(check);
        }
        self.checks = sorted;
        Ok(())
    }
    fn dependencies(&self, check: &Check) -> BTreeSet<Id> {
        let mut dependencies: BTreeSet<_> = check.blocked_by.iter().cloned().collect();
        // Successful observations retain the real preparation graph even
        // though their public blocked_by array is necessarily empty. An early
        // configuration failure instead remains a root; do not invent a cycle.
        let implicit: &[&str] = match check.id.as_str() {
            "context.snapshot" if check.status == Status::Passed => &["context.declared-inputs"],
            "context.configuration"
                if check.status == Status::Passed
                    || (self
                        .context
                        .as_ref()
                        .is_some_and(|context| context.snapshot.is_some())
                        && ["context.declared-inputs", "context.snapshot"]
                            .iter()
                            .all(|id| {
                                self.checks.iter().any(|row| {
                                    row.id.as_str() == *id && row.status == Status::Passed
                                })
                            })) =>
            {
                &["context.declared-inputs", "context.snapshot"]
            }
            "recipe" if check.status != Status::Blocked => &["context.configuration", "tool.madb"],
            "run-host" if check.status != Status::Blocked => &["recipe", "host"],
            "host"
            | "sdk"
            | "resource.installation"
            | "resource.python"
            | "resource.compiler"
            | "tool.madb-source"
                if check.status != Status::Blocked =>
            {
                &["recipe"]
            }
            name if name.starts_with("tool.")
                && name != "tool.madb"
                && check.status != Status::Blocked =>
            {
                &["recipe"]
            }
            _ => &[],
        };
        for name in implicit {
            if self.checks.iter().any(|row| row.id.as_str() == *name) {
                dependencies.insert(Id::new(*name).expect("fixed check id"));
            }
        }
        dependencies
    }
    pub fn output_limit(&self) -> Self {
        let mut short = self.clone();
        short.truncated = true;
        short.exit_code = short.exit_code.max(3);
        short.checks = self
            .checks
            .iter()
            .filter(|c| c.error_kind.is_some())
            .max_by_key(|c| error_exit(c.error_kind.expect("filtered")))
            .cloned()
            .into_iter()
            .collect();
        for check in &mut short.checks {
            check.expected = None;
            check.observed = None;
            check.blocked_by.clear();
            check.diagnostics.truncate(1);
            for d in &mut check.diagnostics {
                d.message = bounded(&d.message, 1024);
            }
        }
        short.diagnostics=vec![Diagnostic{code:Id::new("DOCTOR001").expect("fixed id"),message:"The complete doctor observation exceeded the 1 MiB result budget. This is a reduced error summary, not a complete check inventory.".into(),location:None}];
        short.recovery=vec![Recovery{kind:RecoveryKind::Manual,message:"Preserve the reported error. Inspect a narrower supported input scope; no installation or build was performed.".into(),arguments:None}];
        short
    }
}
/// Compare real final observations to the first read, including initially bad
/// bytes and missing paths. A later repair is still a change during this query.
pub fn reconcile(report: &mut Report, before: Vec<Check>, after: Vec<Check>) -> Result<()> {
    if before.len() != after.len() || before.iter().zip(&after).any(|(a, b)| a.id != b.id) {
        return Err(QueryError::new(
            ErrorKind::Conflict,
            "QUERY002",
            "doctor observations",
            "environment observation inventory changed",
        ));
    }
    for (initial, final_value) in before.into_iter().zip(after) {
        if initial == final_value {
            report.record(initial);
        } else {
            let error = QueryError::new(
                ErrorKind::Conflict,
                "QUERY002",
                initial.subject.clone(),
                "environment observation changed during doctor; retry on a stable installation",
            );
            let mut changed =
                Check::failed(initial.id.as_str(), initial.kind, initial.subject, &error);
            changed.expected = initial.expected;
            changed.observed = final_value.observed;
            report.record(changed);
        }
    }
    Ok(())
}

pub fn required_kind(id: &str) -> Option<CheckKind> {
    Some(match id {
        "context.configuration" => CheckKind::Configuration,
        "context.declared-inputs" | "context.snapshot" => CheckKind::Input,
        "recipe" => CheckKind::Recipe,
        "host" | "run-host" => CheckKind::Host,
        "sdk" => CheckKind::Sdk,
        "resource.installation" | "resource.python" | "resource.compiler" => CheckKind::Resource,
        "tool.madb" | "tool.madb-source" => CheckKind::Tool,
        _ if TOOL_NAMES.iter().any(|name| id == format!("tool.{name}")) => CheckKind::Tool,
        _ => return None,
    })
}
