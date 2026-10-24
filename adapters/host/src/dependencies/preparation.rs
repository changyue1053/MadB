use super::*;
#[derive(Clone, Default)]
pub struct NativePreparationControl {
    discovery: ExecutionControl,
    native: ExecutionControl,
    finish: madb_app::transaction::CommitControl,
}
impl NativePreparationControl {
    pub fn cancel(&self) -> madb_app::process::CancelDecision {
        let decision = self.finish.cancel();
        self.discovery.cancel();
        self.native.cancel();
        decision
    }
}

pub const RECEIPT_FILE: &str = "dependency.json";
pub const PREPROCESSED_FILE: &str = "preprocessed.c";

/// Retained native preparation report. Its final admission gate covers gaps
/// between child processes and cache lookup; it does not authorize an artifact
/// store commit or acknowledge unobserved process cleanup.
#[must_use]
pub struct NativePreparation {
    pub dependency_key: Option<Digest>,
    pub retained_path: Option<PathBuf>,
    pub receipt: Option<FileReference>,
    pub output: Option<FileReference>,
    pub cache_hit: bool,
    pub reason: Option<Finding>,
    discovery: DependencyDiscovery,
    native: Option<RetainedRun<FrozenInputs>>,
    retained: Option<PreparedRun>,
}
impl NativePreparation {
    pub fn executions(&self) -> impl Iterator<Item = &ProcessResult<ProcessExecution>> {
        self.discovery
            .execution()
            .into_iter()
            .chain(self.native.iter().map(RetainedRun::execution))
    }
    pub fn retained_runs(&self) -> impl Iterator<Item = &Path> {
        self.discovery
            .retained_path
            .as_deref()
            .into_iter()
            .chain(self.native.iter().map(RetainedRun::path))
            .chain(self.retained.iter().map(PreparedRun::path))
    }
    pub fn poll_cleanup(&mut self) -> ProcessResult<bool> {
        let discovery = self.discovery.poll_cleanup()?;
        let native = self
            .native
            .as_mut()
            .map_or(Ok(true), RetainedRun::poll_cleanup)?;
        Ok(discovery && native)
    }
}

/// Prepare one freestanding translation unit, retaining the real receipt and
/// output under a new private inputs/ tree. No existing request is edited or
/// silently rebound. The caller must bind returned exact refs before execution.
#[allow(clippy::too_many_arguments)]
pub fn prepare_native<F: DependencyFormat>(
    source_root: &Path,
    tool_root: &Path,
    run_parent: &Path,
    run_name: &LogicalPath,
    request: &DependencyRequest,
    context: Digest,
    format: &F,
    cache: Option<&crate::cache::LocalDependencyCache>,
) -> Result<NativePreparation> {
    prepare_native_controlled(
        source_root,
        tool_root,
        run_parent,
        run_name,
        request,
        context,
        format,
        cache,
        &NativePreparationControl::default(),
    )
}

#[allow(clippy::too_many_arguments)]
pub fn prepare_native_controlled<F: DependencyFormat>(
    source_root: &Path,
    tool_root: &Path,
    run_parent: &Path,
    run_name: &LogicalPath,
    request: &DependencyRequest,
    context: Digest,
    format: &F,
    cache: Option<&crate::cache::LocalDependencyCache>,
    control: &NativePreparationControl,
) -> Result<NativePreparation> {
    let permit = control.finish.begin()?;
    if permit.cancelled() {
        return Err(error(
            "DEP006",
            "prepare",
            "preparation cancelled before discovery",
        ));
    }
    if request.sources.len() != 1 {
        return Err(error(
            "DEP003",
            "prepare",
            "native preparation supports exactly one translation unit",
        ));
    }
    let name = |suffix: &str| {
        LogicalPath::new(format!("{}{suffix}", run_name.as_str()))
            .map_err(|e| error("DEP001", "run-name", e.to_string()))
    };
    let discovery = discover_clang(
        source_root,
        tool_root,
        run_parent,
        &name("-discovery")?,
        request,
        context,
        format,
        &control.discovery,
    )?;
    let mut result = NativePreparation {
        dependency_key: None,
        retained_path: None,
        receipt: None,
        output: None,
        cache_hit: false,
        reason: None,
        discovery,
        native: None,
        retained: None,
    };
    let Some(snapshot) = result.discovery.snapshot.as_ref() else {
        result.reason = result.discovery.reason.clone();
        return Ok(result);
    };
    if permit.cancelled() {
        result.reason = Some(Finding::new(
            "DEP006",
            "prepare",
            "preparation cancelled after discovery",
        ));
        return Ok(result);
    }
    let source = LocalWorkspace::open(source_root)?;
    let tools = LocalWorkspace::open(tool_root)?;
    madb_app::dependencies::verify_compiler(&tools, &request.compiler)?;
    let output = LogicalPath::new(PREPROCESSED_FILE).expect("constant");
    let cached = match cache {
        Some(cache) => cache.load(&source, snapshot, std::slice::from_ref(&output), format)?,
        None => None,
    };
    result.cache_hit = cached.is_some();
    let files = if let Some(files) = cached {
        files
    } else {
        let run = preprocess_clang(
            source_root,
            tool_root,
            run_parent,
            &name("-native")?,
            request,
            snapshot,
            &output,
            &control.native,
        )?;
        let files = run.value().cloned();
        result.native = Some(run);
        let Some(files) = files else {
            result.reason = Some(Finding::new(
                "DEP004",
                "prepare",
                "native preparation failed or cleanup remains pending; retained execution carries details",
            ));
            return Ok(result);
        };
        snapshot.verify(&source)?;
        if let Some(cache) = cache {
            cache.store(snapshot, &files, format)?;
        }
        files
    };
    snapshot.verify(&source)?;
    let mut all = BTreeMap::new();
    all.insert(
        LogicalPath::new(RECEIPT_FILE).expect("constant"),
        snapshot.manifest_bytes().to_vec(),
    );
    for (reference, bytes) in files.files() {
        all.insert(reference.path.clone(), bytes.to_vec());
    }
    let refs = all
        .iter()
        .map(|(path, bytes)| FileReference {
            path: path.clone(),
            digest: Digest::of_bytes(bytes),
            size: bytes.len() as u64,
        })
        .collect();
    let retained_files = capture_inputs(&MemoryFiles(all), refs)?;
    let prepared = PreparedRun::create(run_parent, &name("-prepared")?, retained_files.clone())
        .map_err(retained_preparation_error)?;
    snapshot.verify(&source)?;
    madb_app::dependencies::verify_compiler(&tools, &request.compiler)?;
    result.retained_path = Some(prepared.path().join("inputs"));
    result.dependency_key = Some(snapshot.key().clone());
    result.receipt = retained_files
        .references()
        .find(|f| f.path.as_str() == RECEIPT_FILE)
        .cloned();
    result.output = retained_files
        .references()
        .find(|f| f.path.as_str() == PREPROCESSED_FILE)
        .cloned();
    result.retained = Some(prepared);
    if let Err(error) = permit.publish(|| Ok(())) {
        result.dependency_key = None;
        result.receipt = None;
        result.output = None;
        result.reason = Some(Finding::new(
            "DEP006",
            "prepare",
            format!("preparation result rejected: {error}"),
        ));
    }
    Ok(result)
}
struct MemoryFiles(BTreeMap<LogicalPath, Vec<u8>>);
impl ReadWorkspace for MemoryFiles {
    fn read(&self, path: &LogicalPath, limit: usize) -> Result<Option<Vec<u8>>> {
        match self.0.get(path) {
            Some(bytes) if bytes.len() > limit => Err(error(
                "DEP001",
                "preparation",
                "prepared output exceeds bound",
            )),
            bytes => Ok(bytes.cloned()),
        }
    }
}
