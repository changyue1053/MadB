//! Composition of the supported native preparation provider and receipt checks.
use super::{
    ExecutionOptions, ExecutionSpec, configuration, document, invalid, run_parent, tool_root,
};
use madb_app::{
    ErrorKind, QueryError, Result,
    dependencies::{
        DependencyFormat, PreparedDependencies, preparation_context, verify_prepared_dependencies,
    },
    inputs::FrozenInputs,
};
use madb_contracts::{
    dependencies::JsonDependencyFormat,
    execution::{validate_dependency_target, validate_dependency_tool},
};
use madb_host::{
    LocalWorkspace,
    cache::LocalDependencyCache,
    dependencies::{NativePreparationControl, prepare_native_controlled},
    locations,
};

pub(super) fn prepare(
    options: &ExecutionOptions,
    spec: &ExecutionSpec,
    documents: &FrozenInputs,
    source: &LocalWorkspace,
) -> Result<String> {
    let files = spec.dependencies.as_ref().ok_or_else(|| {
        invalid(
            "dependencies",
            "dependency preparation requires an explicit request",
        )
    })?;
    let format = JsonDependencyFormat;
    let bytes = document(documents, &files.request);
    let request = format.decode_request(bytes)?;
    validate_dependency_tool(configuration(spec, documents), &request.compiler)?;
    validate_dependency_target(configuration(spec, documents), &request.target)?;
    let context = preparation_context(
        bytes,
        document(documents, &spec.context.lock),
        document(documents, &spec.context.profile),
    );
    let parent = run_parent(options, spec)?;
    let cache = spec
        .cache
        .as_ref()
        .map(|path| {
            let path =
                locations::directory(&options.root, path, true)?.expect("created cache directory");
            LocalDependencyCache::open(&path, false)
        })
        .transpose()?;
    let control = NativePreparationControl::default();
    let listener_control = control.clone();
    let _signals = madb_host::cancellation::CancellationSignals::install_action(move || {
        listener_control.cancel();
    })?;
    let preparation = prepare_native_controlled(
        &options.root,
        tool_root(options),
        &parent,
        &locations::unique_run_name(),
        &request,
        context,
        &format,
        cache.as_ref(),
        &control,
    )?;
    documents.verify(source)?;
    if let Some(reason) = &preparation.reason {
        let mut error = QueryError {
            kind: ErrorKind::InvalidInput,
            findings: vec![reason.clone()],
        };
        for path in preparation.retained_runs() {
            error.findings.push(madb_app::Finding::new(
                "RETAINED",
                "preparation",
                path.display().to_string(),
            ));
        }
        for execution in preparation.executions() {
            match execution {
                Err(cause) => error.findings.push(madb_app::Finding::new(
                    "PROCESS",
                    "dependencies",
                    cause.to_string(),
                )),
                Ok(value) => {
                    if let Some(process) = &value.process
                        && let Some(cause) = &process.report.diagnostic
                    {
                        error.findings.push(madb_app::Finding::new(
                            "PROCESS",
                            "dependencies",
                            cause.to_string(),
                        ));
                    }
                }
            }
        }
        return Err(error);
    }
    let directory = preparation
        .retained_path
        .as_ref()
        .ok_or_else(|| invalid("dependencies", "native preparation did not retain a result"))?;
    let receipt = preparation
        .receipt
        .as_ref()
        .expect("accepted preparation receipt");
    let output = preparation
        .output
        .as_ref()
        .expect("accepted preparation output");
    Ok(format!(
        "DependenciesPrepared\nDependencyKey: {}\nReceipt: {}\nOutput: {}\nCache: {}\nExplicitly rebind exact receipt file references before generation.\n",
        preparation
            .dependency_key
            .as_ref()
            .expect("accepted identity")
            .as_str(),
        directory.join(receipt.path.as_str()).display(),
        directory.join(output.path.as_str()).display(),
        if preparation.cache_hit { "hit" } else { "miss" }
    ))
}

pub(super) fn load(
    options: &ExecutionOptions,
    spec: &ExecutionSpec,
    documents: &FrozenInputs,
    source: &LocalWorkspace,
) -> Result<Option<PreparedDependencies>> {
    let Some(files) = &spec.dependencies else {
        return Ok(None);
    };
    let receipt = files.receipt.as_ref().ok_or_else(|| QueryError::new(ErrorKind::InvalidInput, "DEP005", "dependencies", "MetadataRequired: run madb build --task dependencies --execution <file> --frozen, then explicitly bind the returned receipt"))?;
    let format = JsonDependencyFormat;
    let bytes = document(documents, &files.request);
    let request = format.decode_request(bytes)?;
    validate_dependency_tool(configuration(spec, documents), &request.compiler)?;
    validate_dependency_target(configuration(spec, documents), &request.target)?;
    let context = preparation_context(
        bytes,
        document(documents, &spec.context.lock),
        document(documents, &spec.context.profile),
    );
    let tools = LocalWorkspace::open(tool_root(options))?;
    verify_prepared_dependencies(
        source,
        &tools,
        &request,
        document(documents, receipt),
        &context,
        &format,
    )
    .map(Some)
}

pub(super) fn verify(
    options: &ExecutionOptions,
    dependency: &Option<PreparedDependencies>,
    source: &LocalWorkspace,
) -> Result<()> {
    if let Some(dependency) = dependency {
        dependency.verify(source, &LocalWorkspace::open(tool_root(options))?)?;
    }
    Ok(())
}

pub(super) fn validate_domains(spec: &ExecutionSpec, documents: &FrozenInputs) -> Result<()> {
    let mut observed = spec
        .watch
        .directories
        .iter()
        .chain(&spec.watch.absent)
        .cloned()
        .collect::<Vec<_>>();
    if let Some(files) = &spec.dependencies {
        let request = JsonDependencyFormat.decode_request(document(documents, &files.request))?;
        observed.extend(request.include_directories);
        observed.extend(request.absent);
        for source in request.sources {
            let (parent, _) = source.as_str().rsplit_once('/').ok_or_else(|| {
                invalid(
                    "sources",
                    "dependency sources need an explicit parent directory",
                )
            })?;
            observed.push(super::logical(parent)?);
        }
    }
    madb_app::pipeline::validate_observed_locations(
        &spec.store,
        &spec.run_parent,
        spec.cache.as_ref(),
        &observed,
    )
}
