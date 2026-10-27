use super::*;
use serde_json::{Value, json};
use std::collections::BTreeSet;

pub(super) struct OwnedAttempt {
    pub observation: BuildAttempt,
    pub directory: File,
    pub name: String,
}
impl LocalProjectBuild {
    pub(super) fn create_attempt(&mut self, captured: &CapturedInputs) -> Result<BuildAttempt> {
        if !self.baseline_captured || self.created_root.is_some() {
            return Err(conflict(
                "attempt",
                "capture must precede a single owned attempt",
            ));
        }
        if captured.files().len() != self.files.len()
            || captured
                .files()
                .iter()
                .any(|(p, b)| self.files.get(p).is_none_or(|(actual, _)| actual != b))
            || captured.directories() != self.directories
        {
            return Err(conflict(
                "inputs",
                "captured value is not from this workspace",
            ));
        }
        self.verify_sources()?;
        let _state = self
            .open_state(true)?
            .ok_or_else(|| host("state", "state was not created"))?;
        let parent = fs::ensure_directories(&self.directory, ".madb/build/attempts")?;
        let name = fs::nonce();
        let root = AbsolutePath::new(&format!(
            "{}/.madb/build/attempts/{name}",
            self.root.as_str()
        ))
        .map_err(|e| invalid("attempt", e.to_string()))?;
        rustix::fs::mkdirat(
            &parent,
            name.as_str(),
            rustix::fs::Mode::from_bits_retain(0o700),
        )
        .map_err(|e| conflict("attempt creation", e))?;
        // Record the actual mkdir fact before any later open, copy, or sync can
        // fail. This observation never grants authority to resume that attempt.
        self.created_root = Some(root.clone());
        let directory = fs::directory(&parent, &name)?;
        parent
            .sync_all()
            .map_err(|e| host("attempt parent durability", e))?;
        #[cfg(test)]
        if matches!(self.fault, Some(super::publish::Fault::AttemptAfterCreate)) {
            return Err(host("attempt", "injected failure after mkdir"));
        }
        let inputs = fs::create_directory(&directory, "inputs")?;
        for (path, bytes) in captured.files() {
            if let Some((parent, _)) = path.as_str().rsplit_once('/') {
                fs::ensure_directories(&inputs, parent)?;
            }
            fs::write_new(&inputs, path.as_str(), bytes, 0o400)?;
        }
        // Empty source directories are real input conditions and survive copying.
        for d in captured.directories() {
            if d.present {
                fs::ensure_directories(&inputs, d.path.as_str())?;
                for m in &d.members {
                    if m.kind == MemberKind::Directory {
                        fs::ensure_directories(
                            &inputs,
                            &format!("{}/{}", d.path.as_str(), m.path.as_str()),
                        )?;
                    }
                }
            }
        }
        fs::create_directory(&directory, "native")?;
        let inventory = captured.inventory()?;
        fs::write_new(
            &directory,
            ".attempt.json",
            &records::encode(&self.attempt_marker(&inventory))?,
            0o400,
        )?;
        directory.sync_all().map_err(|e| host("attempt", e))?;
        let previous = self
            .baseline
            .as_ref()
            .map(|i| self.previous(i))
            .transpose()?;
        let observation = BuildAttempt {
            root,
            inputs: inventory,
            previous,
        };
        self.attempt = Some(OwnedAttempt {
            observation: observation.clone(),
            directory,
            name,
        });
        self.verify_sources()?;
        self.owned_attempt(&observation)?;
        Ok(observation)
    }
    fn attempt_marker(&self, inputs: &InputInventory) -> Value {
        json!({"kind":"madb-project-attempt","version":1,"origin":Digest::of_bytes(&self.files[&logical(ORIGIN).expect("fixed path")].0).as_str(),
            "files":inputs.files.iter().map(|f|json!({"source":f.source.as_str(),"file":records::file_value(&f.file)})).collect::<Vec<_>>(),
            "directories":inputs.directories.iter().map(|d|json!({"path":d.path.as_str(),"present":d.present,
                "members":d.members.iter().map(|m|json!({"path":m.path.as_str(),"kind":m.kind.as_str()})).collect::<Vec<_>>()})).collect::<Vec<_>>()})
    }
    pub(super) fn owned_attempt(&self, observation: &BuildAttempt) -> Result<&OwnedAttempt> {
        let own = self
            .attempt
            .as_ref()
            .ok_or_else(|| conflict("attempt", "no owned attempt"))?;
        if &own.observation != observation {
            return Err(conflict("attempt", "observation is not this owned attempt"));
        }
        let current = fs::directory(
            &self.directory,
            &format!(".madb/build/attempts/{}", own.name),
        )?;
        if !fs::same_directory(&current, &own.directory)? {
            return Err(conflict("attempt", "owned attempt directory was replaced"));
        }
        self.verify_attempt_marker(&own.directory)?;
        if fs::read(&own.directory, ".attempt.json", MAX_WIRE_BYTES)?.0
            != records::encode(&self.attempt_marker(&own.observation.inputs))?
        {
            return Err(conflict(
                "attempt",
                "owned attempt input record was changed",
            ));
        }
        Ok(own)
    }
    pub(super) fn verify_attempt_marker(&self, directory: &File) -> Result<()> {
        let (bytes, _) = fs::read(directory, ".attempt.json", MAX_WIRE_BYTES)?;
        let v = records::decode(&bytes)?;
        records::keys(&v, &["kind", "version", "origin", "files", "directories"])?;
        let origin = self
            .files
            .get(&logical(ORIGIN)?)
            .ok_or_else(|| conflict("origin", "origin was not captured"))?;
        if v["kind"] != "madb-project-attempt"
            || v["version"] != 1
            || records::digest(&v, "origin")? != Digest::of_bytes(&origin.0)
        {
            return Err(conflict("attempt", "attempt belongs to another origin"));
        }
        let files = v["files"]
            .as_array()
            .filter(|v| v.len() <= 4096)
            .ok_or_else(|| conflict("attempt", "bad input inventory"))?;
        let mut names = BTreeSet::new();
        let mut captured_files = BTreeMap::new();
        let mut total_bytes = 0u64;
        for item in files {
            records::keys(item, &["source", "file"])?;
            let file = records::file(&item["file"])?;
            total_bytes = total_bytes
                .checked_add(file.size)
                .filter(|n| *n <= 128 * 1024 * 1024)
                .ok_or_else(|| {
                    conflict("attempt", "immutable input bytes exceed the source budget")
                })?;
            if file.path.as_str() != format!("inputs/{}", records::text(item, "source")?)
                || !names.insert(file.path.clone())
            {
                return Err(conflict("attempt", "invalid attempt input binding"));
            }
            captured_files.insert(
                logical(records::text(item, "source")?)?,
                verify_file(directory, &file, 32 * 1024 * 1024)?,
            );
        }
        let inputs = fs::directory(directory, "inputs")?;
        let mut directories = Vec::new();
        for path in ["consumer/include", "rust/src"] {
            let (d, observed) = super::capture::scan_at(&inputs, path)?;
            for (source, (bytes, _)) in observed {
                if captured_files.get(&source) != Some(&bytes) {
                    return Err(conflict(
                        source.as_str(),
                        "attempt contains undeclared or changed input bytes",
                    ));
                }
            }
            directories.push(d);
        }
        let captured = CapturedInputs::new(captured_files, directories)?;
        if captured.files().get(&logical(ORIGIN)?) != Some(&origin.0)
            || captured.files().get(&logical(DESCRIPTOR)?) != Some(&self.original)
            || self.attempt_marker(&captured.inventory()?) != v
        {
            return Err(conflict(
                "attempt",
                "attempt input set or provenance is incomplete or inconsistent",
            ));
        }
        Ok(())
    }
    pub(super) fn check_outputs(
        &self,
        attempt: &BuildAttempt,
        output: &ProjectOutput,
    ) -> Result<()> {
        self.verify_sources()?;
        let owned = self.owned_attempt(attempt)?;
        verify_output_tree(&owned.directory, output)
    }
    pub(super) fn previous(&self, index: &records::Index) -> Result<PreviousBuild> {
        let relative = format!(".madb/build/attempts/{}", index.attempt);
        let directory = fs::directory(&self.directory, &relative)?;
        self.verify_attempt_marker(&directory)?;
        let bytes = verify_file(&directory, &index.record, MAX_WIRE_BYTES)?;
        let output = records::output(&bytes)?;
        if output.identity != index.identity {
            return Err(conflict(
                "current",
                "output record and current identity differ",
            ));
        }
        verify_output_tree(&directory, &output)?;
        Ok(PreviousBuild {
            artifact_root: AbsolutePath::new(&format!("{}/{}", self.root.as_str(), relative))
                .map_err(|e| invalid("current", e.to_string()))?,
            identity: index.identity.clone(),
        })
    }
}
pub(super) fn verify_file(root: &File, f: &FileIdentity, limit: usize) -> Result<Vec<u8>> {
    if f.size > limit as u64 {
        return Err(conflict(f.path.as_str(), "file identity exceeds budget"));
    }
    let (bytes, _) = fs::read(root, f.path.as_str(), f.size as usize)?;
    if bytes.len() as u64 != f.size || Digest::of_bytes(&bytes) != f.sha256 {
        return Err(conflict(f.path.as_str(), "actual output bytes differ"));
    }
    Ok(bytes)
}
pub(super) fn verify_output_tree(root: &File, output: &ProjectOutput) -> Result<()> {
    let p = &output.projection;
    let prefix = format!("{}/", p.candidate.root.as_str());
    if !p.candidate.manifest.path.as_str().starts_with(&prefix)
        || p.consumer.path.as_str().starts_with(&prefix)
        || p.outputs.len() > 16384
    {
        return Err(conflict(
            "outputs",
            "candidate and consumer domains are invalid",
        ));
    }
    let mut candidate_references = Vec::new();
    for f in &p.outputs {
        if let Some(relative) = f.path.as_str().strip_prefix(&prefix) {
            candidate_references.push(madb_app::ports::FileReference {
                path: logical(relative)?,
                digest: f.sha256.clone(),
                size: f.size,
            });
        }
    }
    // The SDK still obeys C04's existing finite inventory. The separately linked
    // consumer keeps the existing initialized-executable member limit.
    madb_app::inputs::check_input_inventory(&candidate_references)?;
    let mut names = BTreeSet::new();
    let mut expected = BTreeSet::new();
    for f in &p.outputs {
        if !names.insert(f.path.as_str().to_ascii_lowercase()) {
            return Err(conflict("outputs", "duplicate or aliased output"));
        }
        if f.path != p.consumer.path && !f.path.as_str().starts_with(&prefix) {
            return Err(conflict(
                f.path.as_str(),
                "output is outside candidate or consumer",
            ));
        }
        verify_file(root, f, 256 * 1024 * 1024)?;
        expected.insert(f.path.clone());
    }
    if !p.outputs.contains(&p.candidate.manifest) || !p.outputs.contains(&p.consumer) {
        return Err(conflict(
            "outputs",
            "candidate manifest or consumer identity is missing",
        ));
    }
    let mut actual = BTreeSet::new();
    enumerate_files(
        &fs::directory(root, p.candidate.root.as_str())?,
        p.candidate.root.as_str(),
        &mut actual,
    )?;
    actual.insert(p.consumer.path.clone());
    if actual != expected {
        return Err(conflict(
            "outputs",
            "candidate tree is not the complete declared inventory",
        ));
    }
    let consumer = fs::regular(root, p.consumer.path.as_str())?;
    use std::os::unix::fs::PermissionsExt;
    if consumer
        .metadata()
        .map_err(|e| host("consumer", e))?
        .permissions()
        .mode()
        & 0o111
        == 0
    {
        return Err(conflict("consumer", "consumer is not executable"));
    }
    Ok(())
}
fn enumerate_files(root: &File, prefix: &str, files: &mut BTreeSet<LogicalPath>) -> Result<()> {
    if prefix.split('/').count() > 64 {
        return Err(conflict(prefix, "output depth exceeded"));
    }
    for name in fs::names(root, 16384)? {
        if files.len() >= 16384 {
            return Err(conflict(prefix, "output member budget exceeded"));
        }
        let path = format!("{prefix}/{name}");
        match fs::kind(root, &name)? {
            MemberKind::File => {
                files.insert(logical(&path)?);
            }
            MemberKind::Directory => enumerate_files(&fs::directory(root, &name)?, &path, files)?,
        }
    }
    Ok(())
}
