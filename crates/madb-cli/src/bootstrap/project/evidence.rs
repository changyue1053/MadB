use madb_app::{Digest, Result, ports::FileReference, use_cases::project_build::*};
use madb_host::initialization::LocalInitEvidence;

pub(super) fn allocate() -> Result<LocalInitEvidence> {
    let parent = madb_host::locations::temporary_parent()?;
    let parent = parent
        .to_str()
        .ok_or_else(|| super::invalid("evidence", "temporary path is not UTF-8"))?;
    LocalInitEvidence::allocate(&AbsolutePath::new(parent)?)
}
pub(super) fn save_file(
    store: &mut LocalInitEvidence,
    name: &str,
    bytes: &[u8],
) -> Result<FileIdentity> {
    let path =
        madb_app::LogicalPath::new(name).map_err(|e| super::invalid("evidence", e.to_string()))?;
    let file = FileIdentity {
        path: path.clone(),
        sha256: Digest::of_bytes(bytes),
        size: bytes.len() as u64,
    };
    store.store_file(
        &FileReference {
            path,
            digest: file.sha256.clone(),
            size: file.size,
        },
        bytes,
    )?;
    Ok(file)
}
