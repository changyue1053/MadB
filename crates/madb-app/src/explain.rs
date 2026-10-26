//! Read-only diagnostic explanation. Source binding proves bytes and a point,
//! never that a compiler issued the supplied diagnostic or a candidate passed.
use crate::ports::{FileReference, ReadWorkspace};
use crate::{Digest, ErrorKind, Id, LogicalPath, QueryError, Result};
use madb_model::{Location, contract::Precision};

pub const INPUT_LIMIT: usize = 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PositionEncoding {
    ByteLf,
    Utf16Js,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RepairKind {
    EditAuthorInput,
    InspectConfiguration,
    RestoreLockedInput,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Repair {
    pub kind: RepairKind,
    pub instruction: String,
    pub location: Option<Location>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Explanation {
    pub meaning: String,
    pub triggers: Vec<String>,
    pub repairs: Vec<Repair>,
    pub limits: Vec<String>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocatedSource {
    pub reference: FileReference,
    pub location: Location,
    pub encoding: PositionEncoding,
    pub precision: Option<Precision>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BoundSource {
    pub source: LocatedSource,
    pub byte_offset: u64,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiagnosticContext {
    pub code: Id,
    pub message: String,
    pub source: Option<LocatedSource>,
}
#[derive(Clone, Debug)]
pub struct ExplainRequest {
    pub code: Id,
    pub context: Option<LogicalPath>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExplainSuccess {
    pub explanation: Explanation,
    pub context: Option<FileReference>,
    pub source: Option<BoundSource>,
}

/// The adapter owns schema parsing and its bundled catalog. It cannot probe,
/// run a reader, inspect a workspace or invent a location for an unknown code.
pub trait ExplainFormat {
    fn lookup(&self, code: &Id) -> Result<Option<Explanation>>;
    fn decode_context(&self, bytes: &[u8]) -> Result<DiagnosticContext>;
}

/// No context means no workspace reads, even for an unknown code. Both observed
/// files are reread before success; this is conflict detection, not a snapshot.
pub fn explain<W: ReadWorkspace, F: ExplainFormat>(
    workspace: &W,
    format: &F,
    request: &ExplainRequest,
) -> Result<ExplainSuccess> {
    let mut explanation = format.lookup(&request.code)?.ok_or_else(|| {
        error(
            ErrorKind::InvalidInput,
            "EXPLAIN001",
            request.code.as_str(),
            "this tool version has no explanation entry for the exact code",
        )
    })?;
    let Some(path) = &request.context else {
        explanation
            .limits
            .push("No source context was supplied; no declaration location is claimed.".into());
        return Ok(ExplainSuccess {
            explanation,
            context: None,
            source: None,
        });
    };
    let context_bytes = read(workspace, path)?;
    let context = format.decode_context(&context_bytes)?;
    if context.code != request.code {
        return Err(invalid(
            path,
            "context diagnostic code differs from the query",
        ));
    }
    let mut observed = vec![(path.clone(), context_bytes.clone())];
    let source = if let Some(source) = context.source {
        if source.reference.path != source.location.path {
            return Err(invalid(path, "source and diagnostic location paths differ"));
        }
        let bytes = read(workspace, &source.reference.path)?;
        if source.reference.size != bytes.len() as u64
            || source.reference.digest != Digest::of_bytes(&bytes)
        {
            return Err(invalid(
                &source.reference.path,
                "source bytes differ from the supplied size or digest",
            ));
        }
        let offset = bind_point(
            &bytes,
            source.encoding,
            source.location.line,
            source.location.column,
        )
        .ok_or_else(|| {
            invalid(
                &source.reference.path,
                "location is outside source or splits a UTF-16 scalar",
            )
        })?;
        observed.push((source.reference.path.clone(), bytes));
        for repair in &mut explanation.repairs {
            if repair.kind == RepairKind::EditAuthorInput {
                repair.location = Some(source.location.clone());
            }
        }
        Some(BoundSource {
            source,
            byte_offset: offset as u64,
        })
    } else {
        explanation.limits.push(
            "The supplied diagnostic has no source point; no declaration location is claimed."
                .into(),
        );
        None
    };
    for (path, bytes) in observed {
        match workspace.read(&path, INPUT_LIMIT) {
            Ok(Some(actual)) if actual == bytes => {}
            Err(cause) if cause.kind == ErrorKind::Host => {
                return Err(error(
                    ErrorKind::Host,
                    "EXPLAIN003",
                    path.as_str(),
                    cause.to_string(),
                ));
            }
            Ok(_) => {
                return Err(error(
                    ErrorKind::Conflict,
                    "EXPLAIN004",
                    path.as_str(),
                    "captured input disappeared or its bytes changed during this query",
                ));
            }
            Err(cause) => {
                return Err(error(
                    ErrorKind::Conflict,
                    "EXPLAIN004",
                    path.as_str(),
                    format!("captured input no longer satisfies the read boundary: {cause}"),
                ));
            }
        }
    }
    Ok(ExplainSuccess {
        explanation,
        context: Some(FileReference {
            path: path.clone(),
            digest: Digest::of_bytes(&context_bytes),
            size: context_bytes.len() as u64,
        }),
        source,
    })
}

fn read<W: ReadWorkspace>(workspace: &W, path: &LogicalPath) -> Result<Vec<u8>> {
    let bytes = workspace
        .read(path, INPUT_LIMIT)
        .map_err(|cause| {
            let (kind, code) = match cause.kind {
                ErrorKind::Host => (ErrorKind::Host, "EXPLAIN003"),
                ErrorKind::Conflict => (ErrorKind::Conflict, "EXPLAIN004"),
                ErrorKind::InvalidInput | ErrorKind::CheckFailed => {
                    (ErrorKind::InvalidInput, "EXPLAIN002")
                }
            };
            error(kind, code, path.as_str(), cause.to_string())
        })?
        .ok_or_else(|| {
            error(
                ErrorKind::Host,
                "EXPLAIN003",
                path.as_str(),
                "explicit input file is missing",
            )
        })?;
    if bytes.len() > INPUT_LIMIT {
        return Err(invalid(path, "input exceeds the one MiB limit"));
    }
    Ok(bytes)
}
fn invalid(path: &LogicalPath, message: &str) -> QueryError {
    error(
        ErrorKind::InvalidInput,
        "EXPLAIN002",
        path.as_str(),
        message,
    )
}
fn error(
    kind: ErrorKind,
    code: &'static str,
    subject: &str,
    message: impl Into<String>,
) -> QueryError {
    QueryError::new(kind, code, subject, message)
}

/// Returns an offset in the original bytes. JS coordinates strip exactly one
/// leading BOM; CRLF is one line break and astral scalars occupy two code units.
pub fn bind_point(
    bytes: &[u8],
    encoding: PositionEncoding,
    line: u64,
    column: u64,
) -> Option<usize> {
    if line == 0 || column == 0 {
        return None;
    }
    if encoding == PositionEncoding::ByteLf {
        let mut offset = 0;
        for (index, part) in bytes.split(|byte| *byte == b'\n').enumerate() {
            if index as u64 + 1 == line {
                let column = usize::try_from(column - 1).ok()?;
                return (column <= part.len()).then_some(offset + column);
            }
            offset += part.len() + 1;
        }
        return None;
    }
    let text = std::str::from_utf8(bytes).ok()?;
    let (text, bom) = text.strip_prefix('\u{feff}').map_or((text, 0), |s| (s, 3));
    let mut chars = text.char_indices().peekable();
    let (mut current_line, mut current_column) = (1, 1);
    while let Some((offset, ch)) = chars.next() {
        if current_line == line && current_column == column {
            return Some(bom + offset);
        }
        if ch == '\r' {
            if chars.peek().is_some_and(|(_, next)| *next == '\n') {
                chars.next();
            }
            current_line += 1;
            current_column = 1;
        } else if matches!(ch, '\n' | '\u{2028}' | '\u{2029}') {
            current_line += 1;
            current_column = 1;
        } else {
            current_column += ch.len_utf16() as u64;
        }
    }
    (current_line == line && current_column == column).then_some(bytes.len())
}
