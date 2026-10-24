use super::*;
pub(super) fn search_domain(
    source: &LocalWorkspace,
    r: &DependencyRequest,
) -> Result<(FrozenInputs, InputObservation, Vec<DirectoryDependency>)> {
    let mut pending: BTreeSet<LogicalPath> = r.include_directories.iter().cloned().collect();
    for path in &r.sources {
        pending.insert(
            LogicalPath::new(path.as_str().rsplit_once('/').expect("validated parent").0)
                .map_err(|e| error("DEP001", "source", e.to_string()))?,
        );
    }
    let mut dirs = BTreeMap::new();
    let mut refs = Vec::new();
    let mut members = 0usize;
    let mut names = 0usize;
    let mut total = 0usize;
    while let Some(path) = pending.pop_first() {
        if dirs.contains_key(&path) {
            continue;
        }
        if dirs.len() >= 256 || path.as_str().split('/').count() > 32 {
            return Err(error(
                "DEP001",
                "search-domain",
                "directory/depth budget exceeded",
            ));
        }
        let entries = source.list_directory(&path, 8192 - members, 2 * 1024 * 1024 - names)?;
        members += entries.len();
        names += entries.iter().map(|e| e.name.as_str().len()).sum::<usize>();
        if members > 8192 || names > 2 * 1024 * 1024 {
            return Err(error(
                "DEP001",
                "search-domain",
                "directory member/name budget exceeded",
            ));
        }
        for entry in &entries {
            let child = LogicalPath::new(format!("{}/{}", path.as_str(), entry.name.as_str()))
                .map_err(|e| error("DEP001", "search-domain", e.to_string()))?;
            match entry.kind {
                DirectoryEntryKind::Directory => {
                    pending.insert(child);
                }
                DirectoryEntryKind::File => {
                    if refs.iter().any(|f: &FileReference| f.path == child) {
                        continue;
                    }
                    if refs.len() >= 4096 {
                        return Err(error("DEP001", "search-domain", "file budget exceeded"));
                    }
                    let bytes = source.read(&child, 32 * 1024 * 1024)?.ok_or_else(|| {
                        error("DEP002", child.as_str(), "search-domain file disappeared")
                    })?;
                    total += bytes.len();
                    if total > 128 * 1024 * 1024 {
                        return Err(error(
                            "DEP001",
                            "search-domain",
                            "total bytes budget exceeded",
                        ));
                    }
                    refs.push(FileReference {
                        path: child,
                        digest: Digest::of_bytes(&bytes),
                        size: bytes.len() as u64,
                    });
                }
            }
        }
        dirs.insert(path, entries);
    }
    let watch = InputWatchSet::new(r.absent.clone(), dirs.keys().cloned().collect())?;
    let observation = InputObservation::capture(source, &watch)?;
    let actual: BTreeMap<_, _> = observation
        .directory_members()
        .map(|(p, e)| (p.clone(), e.to_vec()))
        .collect();
    if actual != dirs {
        return Err(error(
            "DEP002",
            "search-domain",
            "directory membership changed during discovery",
        ));
    }
    let files = capture_inputs(source, refs)?;
    observation.verify(source)?;
    Ok((
        files,
        observation,
        dirs.into_iter()
            .map(|(path, entries)| DirectoryDependency { path, entries })
            .collect(),
    ))
}
