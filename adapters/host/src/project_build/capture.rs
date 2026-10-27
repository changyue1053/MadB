use super::*;
use std::collections::BTreeSet;

type CapturedFiles = BTreeMap<LogicalPath, (Vec<u8>, fs::Stamp)>;

/// Retains the actual project root and captured source identities for one
/// approved build or local-tool installation. Observational public paths are
/// never sufficient to adopt another attempt or publication domain.
pub struct LocalProjectBuild {
    pub(super) root: AbsolutePath,
    pub(super) directory: File,
    pub(super) original: Vec<u8>,
    pub(super) files: CapturedFiles,
    pub(super) directories: Vec<InputDirectory>,
    pub(super) directory_stamps: BTreeMap<LogicalPath, fs::Stamp>,
    pub(super) descriptor: Option<Descriptor>,
    pub(super) baseline: Option<records::Index>,
    pub(super) baseline_stamp: Option<fs::Stamp>,
    pub(super) baseline_captured: bool,
    pub(super) attempt: Option<super::attempt::OwnedAttempt>,
    pub(super) created_root: Option<AbsolutePath>,
    #[cfg(test)]
    pub(super) fault: Option<super::publish::Fault>,
}
impl LocalProjectBuild {
    /// Read-only open of the root and raw descriptor. Every traversed component
    /// rejects symlinks; missing, replaced, special, and multiply-linked files
    /// fail before the caller can capture or create an attempt.
    pub fn open(root: &AbsolutePath) -> Result<Self> {
        let directory = fs::absolute(std::path::Path::new(root.as_str()))?;
        let (original, stamp) = fs::read(&directory, DESCRIPTOR, MAX_WIRE_BYTES)?;
        let mut files = BTreeMap::new();
        files.insert(logical(DESCRIPTOR)?, (original.clone(), stamp));
        let value = Self {
            root: root.clone(),
            directory,
            original,
            files,
            directories: Vec::new(),
            directory_stamps: BTreeMap::new(),
            descriptor: None,
            baseline: None,
            baseline_stamp: None,
            baseline_captured: false,
            attempt: None,
            created_root: None,
            #[cfg(test)]
            fault: None,
        };
        value.verify_root()?;
        Ok(value)
    }
    pub(super) fn verify_root(&self) -> Result<()> {
        let current = fs::absolute(std::path::Path::new(self.root.as_str()))?;
        if !fs::same_directory(&self.directory, &current)? {
            return Err(conflict("root", "project directory was replaced"));
        }
        Ok(())
    }
    pub(super) fn freeze(&mut self, path: &str, limit: usize) -> Result<Vec<u8>> {
        let (bytes, stamp) = fs::read(&self.directory, path, limit)?;
        let name = logical(path)?;
        if let Some(old) = self.files.get(&name)
            && old != &(bytes.clone(), stamp.clone())
        {
            return Err(conflict(path, "input changed after capture"));
        }
        self.files.insert(name, (bytes.clone(), stamp));
        Ok(bytes)
    }
    pub(super) fn require_descriptor(&self, d: &Descriptor) -> Result<()> {
        if self.descriptor.as_ref() != Some(d) {
            return Err(conflict(
                "descriptor",
                "origin was not checked for this descriptor",
            ));
        }
        Ok(())
    }
    pub(super) fn scan(&self, path: &str) -> Result<(InputDirectory, CapturedFiles)> {
        scan_at(&self.directory, path)
    }
    pub(super) fn verify_sources(&self) -> Result<()> {
        self.verify_root()?;
        for (name, (bytes, stamp)) in &self.files {
            fs::verify_captured(&self.directory, name.as_str(), bytes, stamp)?;
        }
        for (path, stamp) in &self.directory_stamps {
            if &fs::stamp(&fs::directory(&self.directory, path.as_str())?)? != stamp {
                return Err(conflict(
                    path.as_str(),
                    "captured directory identity changed",
                ));
            }
        }
        for directory in &self.directories {
            let (now, files) = self.scan(directory.path.as_str())?;
            if &now != directory {
                return Err(conflict(
                    directory.path.as_str(),
                    "source member set changed",
                ));
            }
            for (name, current) in files {
                if self.files.get(&name) != Some(&current) {
                    return Err(conflict(name.as_str(), "source member bytes changed"));
                }
            }
        }
        self.verify_root()
    }
}
pub(super) fn scan_at(root: &File, path: &str) -> Result<(InputDirectory, CapturedFiles)> {
    let (parent, name) = fs::parent(root, path)?;
    if !fs::exists(&parent, &name)? {
        return Ok((
            InputDirectory {
                path: logical(path)?,
                present: false,
                members: Vec::new(),
            },
            BTreeMap::new(),
        ));
    }
    let directory = fs::directory(&parent, &name)?;
    let mut members = Vec::new();
    let mut files = BTreeMap::new();
    walk(&directory, path, "", &mut members, &mut files)?;
    Ok((
        InputDirectory {
            path: logical(path)?,
            present: true,
            members,
        },
        files,
    ))
}
fn walk(
    root: &File,
    source: &str,
    prefix: &str,
    members: &mut Vec<DirectoryMember>,
    files: &mut CapturedFiles,
) -> Result<()> {
    if prefix.split('/').count() > 64 {
        return Err(invalid(source, "source directory depth exceeded"));
    }
    for name in fs::names(root, 16384)? {
        if members.len() >= 16384 {
            return Err(invalid(source, "directory member budget exceeded"));
        }
        let relative = if prefix.is_empty() {
            name.clone()
        } else {
            format!("{prefix}/{name}")
        };
        let kind = fs::kind(root, &name)?;
        members.push(DirectoryMember {
            path: logical(&relative)?,
            kind,
        });
        match kind {
            MemberKind::Directory => walk(
                &fs::directory(root, &name)?,
                source,
                &relative,
                members,
                files,
            )?,
            MemberKind::File => {
                if files.len() >= 4096 {
                    return Err(invalid(source, "source file budget exceeded"));
                }
                let value = fs::read(root, &name, 32 * 1024 * 1024)?;
                if files.values().map(|(b, _)| b.len()).sum::<usize>() + value.0.len()
                    > 128 * 1024 * 1024
                {
                    return Err(invalid(source, "source byte budget exceeded"));
                }
                files.insert(logical(&format!("{source}/{relative}"))?, value);
            }
        }
    }
    members.sort_by(|a, b| {
        a.path
            .as_str()
            .encode_utf16()
            .cmp(b.path.as_str().encode_utf16())
    });
    Ok(())
}
impl ProjectWorkspace for LocalProjectBuild {
    fn root(&self) -> &AbsolutePath {
        &self.root
    }
    fn descriptor_bytes(&self) -> Result<Vec<u8>> {
        self.verify_sources()?;
        Ok(self.original.clone())
    }
    fn check_origin(&mut self, d: &Descriptor, original: &[u8]) -> Result<()> {
        self.verify_sources()?;
        records::check_descriptor_paths(d)?;
        if original != self.original {
            return Err(conflict(DESCRIPTOR, "descriptor differs from opened bytes"));
        }
        let origin = self.freeze(ORIGIN, MAX_WIRE_BYTES)?;
        if origin != records::origin_bytes(d, original)? {
            return Err(conflict(
                ORIGIN,
                "missing, unknown or changed initialization provenance",
            ));
        }
        let archive = self.freeze(d.template.archive.path.as_str(), 32 * 1024 * 1024)?;
        if archive.len() as u64 != d.template.archive.size
            || Digest::of_bytes(&archive) != d.template.archive.sha256
        {
            return Err(conflict("template", "original archive identity changed"));
        }
        self.descriptor = Some(d.clone());
        self.verify_sources()
    }
    fn capture(&mut self, d: &Descriptor, original: &[u8]) -> Result<CapturedInputs> {
        self.check_origin(d, original)?;
        if self.attempt.is_some() {
            return Err(conflict(
                "attempt",
                "workspace already owns a build attempt",
            ));
        }
        self.baseline = self.read_baseline()?;
        self.baseline_captured = true;
        for reference in [&d.profile, &d.sdk] {
            let bytes = self.freeze(reference.file.path.as_str(), 32 * 1024 * 1024)?;
            if bytes.len() as u64 != reference.file.size
                || Digest::of_bytes(&bytes) != reference.file.sha256
            {
                return Err(conflict(
                    reference.file.path.as_str(),
                    "descriptor input identity differs",
                ));
            }
        }
        self.freeze("consumer/main.c", 32 * 1024 * 1024)?;
        self.directories.clear();
        self.directory_stamps.clear();
        for path in ["consumer/include", "rust/src"] {
            let (directory, files) = self.scan(path)?;
            self.directories.push(directory);
            self.files.extend(files);
        }
        let mut directory_count = 0;
        for d in &self.directories {
            if d.present {
                self.directory_stamps.insert(
                    d.path.clone(),
                    fs::stamp(&fs::directory(&self.directory, d.path.as_str())?)?,
                );
                for member in &d.members {
                    if member.kind == MemberKind::Directory {
                        directory_count += 1;
                        if directory_count > 8192 {
                            return Err(invalid("inputs", "source directory budget exceeded"));
                        }
                        let path =
                            logical(&format!("{}/{}", d.path.as_str(), member.path.as_str()))?;
                        self.directory_stamps.insert(
                            path.clone(),
                            fs::stamp(&fs::directory(&self.directory, path.as_str())?)?,
                        );
                    }
                }
            }
        }
        let captured = CapturedInputs::new(
            self.files
                .iter()
                .map(|(p, (b, _))| (p.clone(), b.clone()))
                .collect(),
            self.directories.clone(),
        )?;
        let inventory = captured.inventory()?;
        let mut seen = BTreeSet::new();
        for f in &inventory.files {
            if !seen.insert(f.source.as_str().to_ascii_lowercase()) {
                return Err(invalid("inputs", "case alias in captured file inventory"));
            }
        }
        self.verify_sources()?;
        Ok(captured)
    }
    fn created_attempt_root(&self) -> Option<&AbsolutePath> {
        self.created_root.as_ref()
    }
    fn begin_attempt(&mut self, captured: &CapturedInputs) -> Result<BuildAttempt> {
        self.create_attempt(captured)
    }
    fn revalidate(&self) -> Result<()> {
        self.verify_sources()
    }
    fn verify_outputs(&self, attempt: &BuildAttempt, output: &ProjectOutput) -> Result<()> {
        self.check_outputs(attempt, output)
    }
    fn publish(
        &mut self,
        attempt: &BuildAttempt,
        output: &ProjectOutput,
        control: &madb_app::transaction::CommitControl,
    ) -> PublicationObservation {
        self.publish_output(attempt, output, control)
    }
    fn install_tool(
        &mut self,
        d: &Descriptor,
        control: &madb_app::transaction::CommitControl,
    ) -> InstallationObservation {
        self.install_running(d, control)
    }
}
