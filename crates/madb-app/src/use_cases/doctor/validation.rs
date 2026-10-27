use super::*;
use crate::{ErrorKind, Id, QueryError, Result};
use std::collections::BTreeSet;
impl Report {
    pub fn validate(&self) -> Result<()> {
        let invalid =
            |message| QueryError::new(ErrorKind::InvalidInput, "PLAN001", "doctor result", message);
        if self.checks.len() > 4096
            || self.diagnostics.len() > 128
            || self.recovery.len() > 128
            || self.exit_code > 4
        {
            return Err(invalid("result exceeds the declared shape budget"));
        }
        let mut seen = BTreeSet::new();
        let mut failures = BTreeSet::new();
        let mut actual_exit = 0;
        for check in &self.checks {
            if !seen.insert(check.id.clone())
                || check.subject.is_empty()
                || check.subject.chars().count() > 4096
                || check.subject.chars().any(char::is_control)
                || check.diagnostics.len() > 16
            {
                return Err(invalid("invalid or duplicate observation"));
            }
            for d in &check.diagnostics {
                if d.message.is_empty() || d.message.chars().count() > 4096 {
                    return Err(invalid("diagnostic exceeds its budget"));
                }
            }
            match check.status {
                Status::Passed
                    if check.error_kind.is_none()
                        && check.observed.is_some()
                        && check.blocked_by.is_empty() => {}
                Status::Blocked
                    if check.error_kind.is_none()
                        && check.expected.is_none()
                        && check.observed.is_none()
                        && !check.blocked_by.is_empty()
                        && check.blocked_by.windows(2).all(|p| p[0] < p[1])
                        && check.blocked_by.iter().all(|id| failures.contains(id)) =>
                {
                    failures.insert(check.id.clone());
                }
                Status::Failed | Status::Error | Status::Unsupported
                    if check.blocked_by.is_empty() && !check.diagnostics.is_empty() =>
                {
                    let kind = check
                        .error_kind
                        .ok_or_else(|| invalid("failed observation lacks its actual error kind"))?;
                    if !matches!(
                        (check.status, kind),
                        (
                            Status::Failed,
                            ErrorKind::InvalidInput | ErrorKind::CheckFailed
                        ) | (Status::Error, ErrorKind::Host | ErrorKind::Conflict)
                            | (Status::Unsupported, ErrorKind::InvalidInput)
                    ) {
                        return Err(invalid("observation status and actual error kind differ"));
                    }
                    actual_exit = actual_exit.max(error_exit(kind));
                    failures.insert(check.id.clone());
                }
                _ => return Err(invalid("observation status facts are inconsistent")),
            }
        }
        if self.exit_code != actual_exit.max(if self.truncated { 3 } else { 0 }) {
            return Err(invalid(
                "overall exit must equal the actual maximum error category",
            ));
        }
        if let Some(request) = &self.request {
            let required = required_checks(request.verify);
            if !self.truncated && seen.iter().map(Id::to_string).collect::<Vec<_>>() != required {
                return Err(invalid(
                    "report does not contain the exact requested check set",
                ));
            }
            if let Some(context) = &self.context {
                if context.project.path != request.project
                    || context.read_request.file.path != request.read_request
                    || context.read_request.schema.as_str() != "madb.read-request/1"
                    || context.lock.schema.as_str() != "madb.lock/1"
                    || !matches!(
                        context.profile.schema.as_str(),
                        "madb.profile/1" | "madb.profile/2"
                    )
                    || context.snapshot.as_ref().is_some_and(|r| {
                        r.file.path != request.snapshot || r.schema.as_str() != "madb.contract-ir/1"
                    })
                {
                    return Err(invalid(
                        "context identities differ from request or their declared formats",
                    ));
                }
                if let Some(recipe) = &context.recipe {
                    let tool = self
                        .tool
                        .as_ref()
                        .ok_or_else(|| invalid("recipe has no actual loaded tool"))?;
                    if recipe.id.as_str() != madb_plan::project::VALUES_RECIPE
                        || recipe.version != tool.version
                        || recipe.sha256 != tool.sha256
                    {
                        return Err(invalid("recipe is not bound to this loaded tool"));
                    }
                }
            }
        }
        for check in &self.checks {
            if let Some(kind) = required_kind(check.id.as_str()) {
                if check.kind != kind {
                    return Err(invalid("built-in check has the wrong observation kind"));
                }
            } else {
                return Err(invalid("unknown built-in check identity"));
            }
            if check.status == Status::Passed {
                let observed = check
                    .observed
                    .as_ref()
                    .expect("validated observed presence");
                if matches!(
                    observed,
                    Fact::State(State::Missing | State::Unsupported | State::NotChecked)
                ) {
                    return Err(invalid("passed check carries a non-passed state"));
                }
                if check.id.as_str()=="tool.madb" && !self.tool.as_ref().is_some_and(|identity|matches!(observed,Fact::Tool(v) if v.id==identity.id&&v.version==identity.version&&v.sha256==identity.sha256)){return Err(invalid("main observation differs from its actual identity"));}
                if let (Some(Fact::Tool(expected)), Fact::Tool(actual)) =
                    (&check.expected, observed)
                    && expected != actual
                {
                    return Err(invalid("passed tool differs from its expected identity"));
                }
                if check.id.as_str() == "context.snapshot"
                    && let Fact::Bytes { sha256, size } = observed
                    && !self
                        .context
                        .as_ref()
                        .and_then(|c| c.snapshot.as_ref())
                        .is_some_and(|r| r.file.sha256 == *sha256 && r.file.size == *size)
                {
                    return Err(invalid("snapshot byte observation differs from context"));
                }
            }
        }
        if self.truncated
            && (self.exit_code < 3
                || !self
                    .diagnostics
                    .iter()
                    .any(|d| d.code.as_str() == "DOCTOR001"))
        {
            return Err(invalid("truncated report lacks its output-budget failure"));
        }
        if self.exit_code == 0 {
            let request = self
                .request
                .as_ref()
                .ok_or_else(|| invalid("success has no request"))?;
            let context = self
                .context
                .as_ref()
                .ok_or_else(|| invalid("success has no context"))?;
            let tool = self
                .tool
                .as_ref()
                .ok_or_else(|| invalid("success has no observed tool"))?;
            let recipe = context
                .recipe
                .as_ref()
                .ok_or_else(|| invalid("success has no recognized recipe"))?;
            if self.truncated
                || context.snapshot.is_none()
                || recipe.id.as_str() != madb_plan::project::VALUES_RECIPE
                || recipe.version != tool.version
                || recipe.sha256 != tool.sha256
                || self.checks.iter().any(|c| c.status != Status::Passed)
                || !self.diagnostics.is_empty()
            {
                return Err(invalid("success lacks complete bound preflight facts"));
            }
            let actual: Vec<_> = seen.iter().map(Id::to_string).collect();
            if actual != required_checks(request.verify) {
                return Err(invalid(
                    "success does not contain the exact independent required check set",
                ));
            }
        }
        Ok(())
    }
}
