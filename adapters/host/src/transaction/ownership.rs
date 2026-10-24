//! Conservative ownership admission. A nonempty directory is not ours merely
//! because its children happen to be named staging or batches.
use super::*;
use rustix::fs::flock;

const OWNER: &str = ".owner.json";
const SCHEMA: &str = "madb.storage-owner/1";

/// Fresh directory open: initialization locks coordinate first openers without
/// creating a lock file in an unknown directory. Order is init -> domain only.
pub(super) fn initialization(root: &File, operation: FlockOperation) -> Result<File> {
    let lock = fs::directory(root, ".")?;
    flock(&lock, operation).map_err(|e| io_error("store initialization", e))?;
    Ok(lock)
}
pub(super) fn verify(root: &File) -> Result<()> {
    if codec::read(root, OWNER)? != Some(serde_json::json!({"schema":SCHEMA})) {
        return Err(fail(
            "STORE004",
            "domain",
            "nonempty directory lacks a valid MadB owner marker; initialization is unknown or incomplete",
        ));
    }
    Ok(())
}
pub(super) fn establish(root: &File) -> Result<()> {
    let file = fs::write(
        root,
        OWNER,
        &codec::encode(&serde_json::json!({"schema":SCHEMA}))?,
    )?;
    file.sync_all().map_err(|e| io_error("owner marker", e))?;
    root.sync_all()
        .map_err(|e| io_error("owner marker directory", e))
}
pub(super) fn nonce_name(value: &str) -> bool {
    let Some((pid, serial)) = value.split_once('-') else {
        return false;
    };
    !pid.is_empty()
        && !serial.is_empty()
        && pid.bytes().all(|byte| byte.is_ascii_digit())
        && serial.bytes().all(|byte| byte.is_ascii_digit())
}
pub(super) fn index_temporary(name: &str) -> bool {
    name.strip_prefix(".index-")
        .and_then(|name| name.strip_suffix(".tmp"))
        .is_some_and(nonce_name)
}
pub(super) fn validate_root(root: &File) -> Result<()> {
    verify(root)?;
    for name in fs::names(root, ENTRY_LIMIT)? {
        match name.as_str() {
            "staging" | "batches" => {
                fs::directory(root, &name)?;
            }
            ".domain-lock" | ".owner.json" | "current.json" => {
                fs::regular(root, &name, false, false)?;
            }
            _ if index_temporary(&name) => {
                fs::regular(root, &name, false, false)?;
            }
            _ => {
                return Err(fail(
                    "STORE004",
                    &name,
                    "unknown output-domain entry; refusing ownership or cleanup",
                ));
            }
        }
    }
    Ok(())
}
/// Preflight every managed namespace before deleting anything. Unknown names and
/// links are retained as errors, even when an earlier orphan was collectible.
pub(super) fn namespace(root: &File, category: &str) -> Result<Vec<String>> {
    let directory = fs::directory(root, category)?;
    let names = fs::names(&directory, ENTRY_LIMIT)?;
    for name in &names {
        let recognized = if category == "staging" {
            nonce_name(name)
        } else {
            name.len() == 64
                && name
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        };
        if !recognized {
            return Err(fail(
                "STORE004",
                name,
                "unknown managed namespace entry; refusing cleanup",
            ));
        }
        let batch = fs::directory(&directory, name)?;
        if fs::exists(&batch, ".lease")? {
            fs::regular(&batch, ".lease", false, false)?;
        }
    }
    Ok(names)
}
