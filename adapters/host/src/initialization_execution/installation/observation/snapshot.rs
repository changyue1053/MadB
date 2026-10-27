//! Bounded actual observations, including initially incorrect bytes. These are
//! private conflict-detection facts, never installation/execution authority.
use super::*;
use std::collections::BTreeSet;

pub(super) struct Snapshot {
    pub signature: Digest,
    pub error: Option<madb_app::QueryError>,
}
pub(super) fn record(value: &Value) -> Snapshot {
    let mut paths = BTreeSet::new();
    fn paths_in(value: &Value, paths: &mut BTreeSet<PathBuf>) {
        match value {
            Value::Object(fields) => {
                for (key, value) in fields {
                    if key == "path" && fields.contains_key("image_path") {
                        continue;
                    }
                    if matches!(key.as_str(), "path" | "image_path" | "root") {
                        if let Some(text) = value.as_str()
                            && text.starts_with('/')
                        {
                            paths.insert(text.into());
                        }
                    } else if key == "absent_paths" {
                        if let Some(values) = value.as_array() {
                            for path in values {
                                if let Some(path) = path.as_str() {
                                    paths.insert(path.into());
                                }
                            }
                        }
                    } else {
                        paths_in(value, paths);
                    }
                }
            }
            Value::Array(values) => {
                for value in values {
                    paths_in(value, paths);
                }
            }
            _ => {}
        }
    }
    paths_in(value, &mut paths);
    let excluded = value
        .get("excluded_components")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect::<BTreeSet<_>>();
    let absent = value
        .get("absent_paths")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(PathBuf::from)
        .collect();
    capture(paths, &excluded, &absent)
}
pub(super) fn files(files: impl Iterator<Item = PathBuf>) -> Snapshot {
    capture(files.collect(), &BTreeSet::new(), &BTreeSet::new())
}
#[cfg(unix)]
fn capture(
    paths: BTreeSet<PathBuf>,
    excluded: &BTreeSet<&str>,
    absent: &BTreeSet<PathBuf>,
) -> Snapshot {
    use std::os::unix::fs::MetadataExt;
    let mut observations = Vec::new();
    let mut error = None;
    let mut total = 0u64;
    let mut ancestors = BTreeSet::new();
    if paths.len() > 24_096 {
        return failed(invalid(
            "doctor resources",
            "observation entry budget exceeded",
        ));
    }
    for path in paths {
        // Bind every named ancestor identity without observing unrelated names.
        for ancestor in path.ancestors().skip(1) {
            if !ancestors.insert(ancestor.to_owned()) {
                continue;
            }
            match std::fs::symlink_metadata(ancestor) {
                Ok(m) => {
                    observations.push(serde_json::json!([ancestor, m.dev(), m.ino(), m.mode()]))
                }
                Err(e) => {
                    observations.push(serde_json::json!([ancestor, e.to_string()]));
                    if error.is_none() {
                        error = Some(host("resource ancestor", e));
                    }
                }
            }
        }
        let metadata = match std::fs::symlink_metadata(&path) {
            Ok(v) => v,
            Err(e) => {
                observations.push(serde_json::json!([path, e.to_string()]));
                if !(e.kind() == std::io::ErrorKind::NotFound && absent.contains(&path))
                    && error.is_none()
                {
                    error = Some(host("resource file", e));
                }
                continue;
            }
        };
        let mut row = serde_json::json!([
            path,
            metadata.dev(),
            metadata.ino(),
            metadata.mode(),
            metadata.nlink()
        ]);
        let result = (|| -> Result<Value> {
            if metadata.is_file() {
                total = total
                    .checked_add(metadata.len())
                    .ok_or_else(|| invalid("doctor resources", "observation size overflow"))?;
                if total > INSTALLATION_BYTES || metadata.len() > 256 * 1024 * 1024 {
                    return Err(invalid(
                        "doctor resources",
                        "actual resource bytes exceed observation budget",
                    ));
                }
                let root = LocalWorkspace::open(Path::new("/"))?;
                let bytes = root
                    .read_with_policy(
                        &crate::initialization_execution::absolute_relative(&path)?,
                        256 * 1024 * 1024,
                        true,
                    )?
                    .ok_or_else(|| conflict("resource", "file disappeared while observing"))?;
                Ok(serde_json::json!([
                    Digest::of_bytes(&bytes).as_str(),
                    bytes.len()
                ]))
            } else if metadata.is_symlink() {
                Ok(serde_json::json!(
                    std::fs::read_link(&path).map_err(|e| host("resource alias", e))?
                ))
            } else if metadata.is_dir() {
                let mut names = Vec::new();
                for entry in std::fs::read_dir(&path).map_err(|e| host("resource directory", e))? {
                    let name = entry
                        .map_err(|e| host("resource directory", e))?
                        .file_name();
                    if name.to_str().is_some_and(|v| excluded.contains(v)) {
                        continue;
                    }
                    if names.len() >= 24_096 {
                        return Err(invalid(
                            "doctor resources",
                            "directory entry budget exceeded",
                        ));
                    }
                    names.push(name);
                }
                names.sort();
                Ok(serde_json::json!(names))
            } else {
                Ok(serde_json::json!("non-regular"))
            }
        })();
        match result {
            Ok(value) => row.as_array_mut().expect("array").push(value),
            Err(e) => {
                row.as_array_mut()
                    .expect("array")
                    .push(serde_json::json!(e.to_string()));
                if error.is_none() {
                    error = Some(e);
                }
            }
        }
        observations.push(row);
    }
    Snapshot {
        signature: Digest::of_bytes(
            &serde_json::to_vec(&observations).expect("bounded JSON facts"),
        ),
        error,
    }
}
#[cfg(not(unix))]
fn capture(_: BTreeSet<PathBuf>, _: &BTreeSet<&str>, _: &BTreeSet<PathBuf>) -> Snapshot {
    failed(host("platform", "doctor resources require Unix"))
}
fn failed(error: madb_app::QueryError) -> Snapshot {
    Snapshot {
        signature: Digest::of_bytes(error.to_string().as_bytes()),
        error: Some(error),
    }
}
