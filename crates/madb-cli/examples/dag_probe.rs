//! Test-only composition root for independently specified, real multi-node DAGs.
use madb_app::{
    Digest, ErrorKind, Id, LogicalPath, QueryError, Result,
    dag::{
        GraphControl, GraphExecutor, GraphRecipe, NodeRun, execute_graph, execute_graph_observed,
    },
    input_observation::{DirectoryEntry, InputWatchSet, ObserveWorkspace},
    inputs::FrozenInputs,
    ports::{ConfigurationBytes, FileReference, ReadWorkspace},
    process::{ExecutionControl, ProcessExecution, ProcessPolicy, ProcessResult},
    task::ValidatedTaskResult,
};
use madb_contracts::task::{TaskBinding, bind_graph};
use madb_host::{
    LocalWorkspace,
    dag::{LocalGraphExecutor, LocalNodeRun},
};
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    error::Error,
    fs,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

struct ObservedExecutor<'a> {
    inner: LocalGraphExecutor,
    control: &'a GraphControl,
    source: &'a Path,
    action: &'a str,
    count: usize,
    completed: &'a Cell<usize>,
    cancel_decision: Option<String>,
}
impl<R: GraphRecipe> GraphExecutor<R> for ObservedExecutor<'_> {
    type Run = LocalNodeRun;
    fn execute(
        &mut self,
        recipe: &R,
        inputs: FrozenInputs,
        control: &ExecutionControl,
        result_limit: usize,
    ) -> Self::Run {
        let run = self.inner.execute(recipe, inputs, control, result_limit);
        self.count += 1;
        self.completed.set(self.count);
        if self.count == 1 {
            match self.action {
                "change-source" => {
                    fs::write(self.source.join("later.bin"), b"changed external source").unwrap()
                }
                "change-output" => fs::write(
                    run.path().unwrap().join("work/a.bin"),
                    b"changed live output",
                )
                .unwrap(),
                "watch-optional" => {
                    fs::write(self.source.join("watched/optional.h"), b"new optional").unwrap()
                }
                "watch-add" => fs::write(self.source.join("watched/new.h"), b"new member").unwrap(),
                "watch-remove" => fs::remove_file(self.source.join("watched/keep.h")).unwrap(),
                "watch-type" => {
                    fs::remove_file(self.source.join("watched/keep.h")).unwrap();
                    fs::create_dir(self.source.join("watched/keep.h")).unwrap();
                }
                "watch-content" => fs::write(
                    self.source.join("watched/keep.h"),
                    b"changed untracked content",
                )
                .unwrap(),
                "watch-nested" => {
                    fs::write(self.source.join("watched/nested/new.h"), b"nested member").unwrap()
                }
                "watch-unrelated" => {
                    fs::write(self.source.join("unwatched/new.h"), b"unrelated").unwrap()
                }
                "cancel-after-first" => {
                    self.cancel_decision = Some(format!("{:?}", self.control.cancel()))
                }
                "replace-graph" => {
                    fs::rename(self.inner.path(), self.inner.path().with_extension("moved"))
                        .unwrap();
                    fs::create_dir(self.inner.path()).unwrap();
                }
                _ => {}
            }
        }
        run
    }
}

/// Test-only, deterministic observation windows around real host IO. The
/// graph's completed-node count distinguishes initial capture from the final
/// check without depending on the number of internal verification passes.
struct ProbeWorkspace<'a> {
    inner: LocalWorkspace,
    control: &'a GraphControl,
    completed: &'a Cell<usize>,
    expected_nodes: usize,
    hook: &'a str,
    hook_fired: Cell<bool>,
    hook_cancel: RefCell<Option<String>>,
    reads: Cell<usize>,
    exists: Cell<usize>,
    listings: Cell<usize>,
}
impl ReadWorkspace for ProbeWorkspace<'_> {
    fn read(&self, path: &LogicalPath, limit: usize) -> Result<Option<Vec<u8>>> {
        self.reads.set(self.reads.get() + 1);
        self.inner.read(path, limit)
    }
}
impl ObserveWorkspace for ProbeWorkspace<'_> {
    fn exists(&self, path: &LogicalPath) -> Result<bool> {
        self.exists.set(self.exists.get() + 1);
        self.inner.exists(path)
    }
    fn list_directory(
        &self,
        path: &LogicalPath,
        entry_limit: usize,
        name_bytes_limit: usize,
    ) -> Result<Vec<DirectoryEntry>> {
        self.listings.set(self.listings.get() + 1);
        let at_capture = self.completed.get() == 0;
        let at_final = self.completed.get() == self.expected_nodes;
        if !self.hook_fired.get()
            && ((at_capture && self.hook == "cancel-capture")
                || (at_final && matches!(self.hook, "cancel-final" | "error-final")))
        {
            self.hook_fired.set(true);
            if self.hook == "error-final" {
                return Err(QueryError::new(
                    ErrorKind::Host,
                    "TEST_OBSERVE_IO",
                    "directory",
                    "injected final directory read failure",
                ));
            }
            *self.hook_cancel.borrow_mut() = Some(format!("{:?}", self.control.cancel()));
        }
        self.inner
            .list_directory(path, entry_limit, name_bytes_limit)
    }
}

fn string<'a>(value: &'a Value, key: &str) -> &'a str {
    value[key].as_str().expect("fixture string")
}
fn query_error(phase: &str, error: &QueryError, retained: Option<&Path>) -> Value {
    json!({"phase":phase,"codes":error.findings.iter().map(|f|f.code).collect::<Vec<_>>(),
           "message":error.to_string(),"retained_path":retained})
}
fn files(result: &ValidatedTaskResult) -> Value {
    let encode = |(r, bytes): (&FileReference, &[u8])| json!({"path":r.path.as_str(),"size":r.size,"sha256":r.digest.as_str(),"bytes":bytes});
    json!({"outputs":result.outputs().map(encode).collect::<Vec<_>>(),
           "logs":result.logs().map(encode).collect::<Vec<_>>()})
}
fn execution(execution: &ProcessResult<ProcessExecution>) -> Value {
    match execution {
        Err(error) => {
            json!({"error":error.code,"stage":error.stage,"source":error.source().map(ToString::to_string)})
        }
        Ok(execution) => match &execution.process {
            None => json!({"admission":format!("{:?}",execution.admission),"started":false}),
            Some(run) => json!({
                "admission":format!("{:?}",execution.admission),"started":true,
                "termination":format!("{:?}",run.report.termination),
                "diagnostic":run.report.diagnostic.as_ref().map(|e|e.code),
                "diagnostic_source":run.report.diagnostic.as_ref().and_then(|e|e.source()).map(ToString::to_string),
                "leader_reaped":run.report.cleanup.leader_reaped,
                "cleanup_complete":run.report.cleanup.locally_complete(),
                "pending_cleanup":run.pending_cleanup.is_some(),
            }),
        },
    }
}
fn run(config: &Value) -> Value {
    let source = PathBuf::from(string(config, "source_root"));
    let parent = PathBuf::from(string(config, "run_parent"));
    let tool = PathBuf::from(string(&config["tool"], "path"));
    let action = config["action"].as_str().unwrap_or("none");
    let project = fs::read(source.join("project.json")).unwrap();
    let request = fs::read(source.join("read.json")).unwrap();
    let lock = fs::read(source.join("madb.lock")).unwrap();
    let profile = fs::read(source.join("profiles/rust-c.json")).unwrap();
    let ir = fs::read(source.join("contract.json")).unwrap();
    let tasks = config["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|name| fs::read(source.join(name.as_str().unwrap())).unwrap())
        .collect::<Vec<_>>();
    let installed_tool = FileReference {
        path: LogicalPath::new(tool.file_name().unwrap().to_str().unwrap()).unwrap(),
        digest: Digest::new(string(&config["tool"], "sha256")).unwrap(),
        size: config["tool"]["size"].as_u64().unwrap(),
    };
    let target = config["target"]
        .as_str()
        .map(|value| Id::new(value).unwrap());
    let graph = match bind_graph(
        ConfigurationBytes {
            request: &request,
            project: &project,
            lock: &lock,
            profile: &profile,
        },
        &ir,
        tasks
            .iter()
            .map(|task| TaskBinding {
                task,
                installed_tool: installed_tool.clone(),
            })
            .collect(),
        target.as_ref(),
        &[],
    ) {
        Ok(graph) => graph,
        Err(error) => return query_error("bind", &error, None),
    };
    let watch = if config["watch"].is_object() {
        let paths = |key: &str| {
            config["watch"][key]
                .as_array()
                .unwrap()
                .iter()
                .map(|value| LogicalPath::new(value.as_str().unwrap()).unwrap())
                .collect()
        };
        match InputWatchSet::new(paths("absent"), paths("directories")) {
            Ok(watch) => Some(watch),
            Err(error) => return query_error("watch", &error, None),
        }
    } else {
        None
    };
    let control = GraphControl::default();
    let inner = match LocalGraphExecutor::new(
        &parent,
        &LogicalPath::new("graph").unwrap(),
        tool.parent().unwrap(),
        ProcessPolicy {
            timeout: Duration::from_secs(3),
            termination_grace: Duration::from_millis(100),
            cleanup_timeout: Duration::from_millis(300),
            stdout_limit: 1024 * 1024,
            stderr_limit: 4096,
            ..Default::default()
        },
    ) {
        Ok(inner) => inner,
        Err(error) => return query_error("prepare", &error.error, error.retained_path.as_deref()),
    };
    let graph_path = inner.path().to_owned();
    if action == "node-collision" {
        fs::create_dir(graph_path.join("node-0001")).unwrap();
        fs::write(
            graph_path.join("node-0001/existing"),
            b"unrelated existing directory",
        )
        .unwrap();
    }
    if action == "cancel-before" {
        control.cancel();
    }
    let finished = Arc::new(AtomicBool::new(false));
    let observer = if action == "cancel-running" {
        let marker = PathBuf::from(string(config, "marker"));
        let control = control.clone();
        let finished = finished.clone();
        Some(std::thread::spawn(move || {
            let start = Instant::now();
            loop {
                if fs::read(&marker)
                    .ok()
                    .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
                    .is_some_and(|v| v["ready"] == true)
                {
                    break;
                }
                assert!(
                    !finished.load(Ordering::Acquire),
                    "fixture ended before readiness"
                );
                assert!(
                    start.elapsed() < Duration::from_secs(5),
                    "readiness deadline"
                );
                std::thread::sleep(Duration::from_millis(5));
            }
            control.cancel()
        }))
    } else {
        None
    };
    let completed = Cell::new(0);
    let mut executor = ObservedExecutor {
        inner,
        control: &control,
        source: &source,
        action,
        count: 0,
        completed: &completed,
        cancel_decision: None,
    };
    let workspace = ProbeWorkspace {
        inner: LocalWorkspace::open(&source).unwrap(),
        control: &control,
        completed: &completed,
        expected_nodes: graph.tasks().len(),
        hook: config["watch_hook"].as_str().unwrap_or("none"),
        hook_fired: Cell::new(false),
        hook_cancel: RefCell::new(None),
        reads: Cell::new(0),
        exists: Cell::new(0),
        listings: Cell::new(0),
    };
    let executed = if let Some(watch) = &watch {
        execute_graph_observed(&graph, &workspace, watch, &mut executor, &control)
    } else {
        execute_graph(&graph, &workspace, &mut executor, &control)
    };
    let mut run = match executed {
        Ok(run) => run,
        Err(error) => return query_error("admission", &error, Some(&graph_path)),
    };
    finished.store(true, Ordering::Release);
    let observer_cancel = observer.map(|observer| format!("{:?}", observer.join().unwrap()));
    let nodes = run.nodes().iter().map(|(id,node)| json!({
        "id":id.as_str(),"path":node.path(),"execution":node.execution().map(execution),
        "preparation_error":node.preparation_error().map(|e|query_error("prepare",&e.error,e.retained_path.as_deref()))
    })).collect::<Vec<_>>();
    let values = run.results().map(|results| {
        results
            .iter()
            .map(|(id, value)| (id.as_str().to_owned(), files(value)))
            .collect::<serde_json::Map<_, _>>()
    });
    let values_drained = run
        .nodes_mut()
        .iter_mut()
        .all(|(_, node)| node.take_value().is_none());
    let mut result = json!({"phase":"execute","status":format!("{:?}",run.status),
        "failure":run.failure.as_ref().map(|e|query_error("graph",e,None)),
        "failed_task":run.failed_task.as_ref().map(Id::as_str),
        "skipped":run.skipped.iter().map(Id::as_str).collect::<Vec<_>>(),"nodes":nodes,"results":values,
        "values_drained":values_drained,"retained_path":graph_path,
        "observer_cancel":observer_cancel,"between_cancel":executor.cancel_decision,
        "watch_cancel":workspace.hook_cancel.borrow().as_ref(),"watch_hook_fired":workspace.hook_fired.get(),
        "source_reads":workspace.reads.get(),"exists_reads":workspace.exists.get(),"directory_reads":workspace.listings.get(),
        "late_cancel":format!("{:?}",control.cancel())});
    drop(run);
    drop(executor);
    result["retained_after_drop"] = json!(graph_path.is_dir());
    result
}
fn main() {
    let config: Value = serde_json::from_slice(
        &fs::read(std::env::args().nth(1).expect("fixture config")).unwrap(),
    )
    .unwrap();
    let start = Instant::now();
    let result = run(&config);
    println!(
        "{}",
        json!({"result":result,"elapsed_ms":start.elapsed().as_millis()})
    );
}
