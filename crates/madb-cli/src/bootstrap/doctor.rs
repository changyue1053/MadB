//! Doctor composition: all inputs are read through bounded read-only adapters.
use super::init::{catalog, prepare};
use crate::doctor::DoctorOptions;
use madb_app::use_cases::{
    doctor::*,
    initialize::{ToolObservation, VerifyLevel},
};
use madb_app::{ErrorKind, Id, QueryError, Result};
use madb_contracts::doctor::DoctorAdapter;
use madb_contracts::project_init::ProjectInitAdapter;
use madb_host::{
    doctor::DoctorWorkspace,
    initialization_execution::InstallationObserver,
    initialization_source::{
        FrozenInitializationTool, observe_initialization_host, observe_initialization_system_build,
    },
};
use madb_plan::project::{SelectedTemplate, VALUES_RECIPE, Version};

pub struct DoctorService;
impl DoctorService {
    pub fn run(options: Result<DoctorOptions>) -> (Report, Vec<u8>) {
        let options = match options {
            Ok(v) => v,
            Err(error) => return madb_contracts::doctor::early_error(&error),
        };
        let adapter = match DoctorAdapter::new() {
            Ok(v) => v,
            Err(error) => return madb_contracts::doctor::early_error(&error),
        };
        let mut report = Report::default();
        if let Err(error) = inspect(&options, &adapter, &mut report) {
            report.fail(&error);
        }
        if let Err(error) = report.finish() {
            report.fail(&error);
            report.recovery();
        }
        match adapter.encode_result(&report) {
            Ok(value) => value,
            Err(error) => madb_contracts::doctor::early_error(&error),
        }
    }
}
fn inspect(options: &DoctorOptions, adapter: &DoctorAdapter, report: &mut Report) -> Result<()> {
    let tool = FrozenInitializationTool::capture(env!("CARGO_PKG_VERSION"))?;
    report.tool = Some(tool.identity().clone());
    let self_check = match Version::parse(&tool.identity().version) {
        Ok(_) => Check::passed(
            "tool.madb",
            CheckKind::Tool,
            "actual loaded MadB image",
            None,
            Fact::Tool(ToolObservation {
                id: tool.identity().id.clone(),
                version: tool.identity().version.clone(),
                sha256: tool.identity().sha256.clone(),
                host: id(&format!("{}-apple-darwin", std::env::consts::ARCH)),
                target: id(&format!("{}-apple-darwin", std::env::consts::ARCH)),
                source: "system://loaded-madb-image".into(),
                arguments: vec![],
            }),
        ),
        Err(e) => Check::failed(
            "tool.madb",
            CheckKind::Tool,
            "actual MadB version",
            &invalid(e.to_string()),
        ),
    };
    report.record(self_check);
    let workspace = match DoctorWorkspace::open(&options.root) {
        Ok(value) => value,
        Err(error) => {
            if let Err(changed) = tool.revalidate() {
                report.record(Check::failed(
                    "tool.madb",
                    CheckKind::Tool,
                    "actual loaded image",
                    &changed,
                ));
            }
            return Err(error);
        }
    };
    let request = Request {
        root: workspace.root().clone(),
        project: options.project.clone(),
        read_request: options.read_request.clone(),
        snapshot: options.snapshot.clone(),
        verify: options.verify,
    };
    report.request = Some(request.clone());
    let mut query = DoctorQuery::new(workspace.workspace());
    let checked = query.check(adapter, &request, report);
    let mut environment = None;

    if let Some(configuration) = checked {
        match resolve(&tool, &configuration, &mut query, report) {
            Ok(selected) => {
                report.context.as_mut().expect("checked context").recipe =
                    Some(ComponentIdentity {
                        id: id(VALUES_RECIPE),
                        version: tool.identity().version.clone(),
                        sha256: tool.identity().sha256.clone(),
                    });
                report.record(Check::passed(
                    "recipe",
                    CheckKind::Recipe,
                    VALUES_RECIPE,
                    None,
                    Fact::State(State::Supported),
                ));
                let mut first = host_checks(&selected, request.verify);
                if request.verify != VerifyLevel::Structure {
                    match InstallationObserver::load() {
                        Ok(observer) => {
                            if let Err(error) = check_requirements(
                                &selected,
                                request.verify,
                                &observer.observations(),
                            ) {
                                report.record(Check::failed(
                                    "recipe",
                                    CheckKind::Recipe,
                                    VALUES_RECIPE,
                                    &error,
                                ));
                                report.context.as_mut().expect("context").recipe = None;
                            } else {
                                let facts = observer.capture();
                                first.extend(facts.checks().iter().cloned());
                                environment = Some((observer, facts));
                            }
                        }
                        Err(error) => report.record(Check::failed(
                            "sdk",
                            CheckKind::Sdk,
                            "compiled installation record",
                            &error,
                        )),
                    }
                    let source = option_env!("MADB_SOURCE_REVISION").filter(|v| {
                        v.len() == 40
                            && v.bytes()
                                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
                    });
                    report.record(match source {Some(_)=>Check::passed("tool.madb-source",CheckKind::Tool,"compiled source revision",None,Fact::State(State::Checked)),None=>Check::failed("tool.madb-source",CheckKind::Tool,"compiled source revision",&QueryError::new(ErrorKind::Host,"HOST005","tool.source","this compiled tool lacks its exact source revision; install a source-bound candidate"))});
                }
                for check in &first {
                    report.record(check.clone());
                }
                let mut second = host_checks(&selected, request.verify);
                if let Some((observer, facts)) = &environment {
                    second.extend(observer.reobserve(facts));
                }
                if let Err(error) = reconcile(report, first, second) {
                    report.fail(&error);
                }
            }
            Err(error) => {
                let mut check = Check::failed("recipe", CheckKind::Recipe, VALUES_RECIPE, &error);
                if error.kind == ErrorKind::InvalidInput
                    && error
                        .findings
                        .iter()
                        .any(|f| f.subject == "doctor unsupported recipe")
                {
                    check.status = Status::Unsupported;
                }
                report.record(check);
            }
        }
    }
    // Always recheck even when initial parsing, metadata or environment failed.
    query.revalidate(report);
    if let Err(error) = workspace.revalidate() {
        report.record(Check::failed(
            "context.configuration",
            CheckKind::Configuration,
            "held workspace root",
            &error,
        ));
    }
    if let Err(error) = tool.revalidate() {
        report.record(Check::failed(
            "tool.madb",
            CheckKind::Tool,
            "actual loaded image",
            &error,
        ));
    }
    fill_blocked(report, request.verify);
    Ok(())
}
fn resolve<W: madb_app::ports::ReadWorkspace>(
    tool: &FrozenInitializationTool,
    configuration: &CheckedConfiguration,
    query: &mut DoctorQuery<'_, W>,
    _report: &mut Report,
) -> Result<SelectedTemplate> {
    let format = ProjectInitAdapter::new()?;
    let record = catalog::installed(tool.identity(), Some("org.madb.template/rust-c-sdk@0.1.1"))?;
    record.recipe(&id(VALUES_RECIPE))?;
    let (template, inventory) = prepare::load(&record, &format)?;
    template
        .check_madb_version(&tool.identity().version)
        .map_err(|e| invalid(e.to_string()))?;
    let selected = template
        .select(None, None)
        .map_err(|e| invalid(e.to_string()))?;
    let boundary = selected
        .template()
        .boundary_refs
        .iter()
        .find(|r| selected.profile().boundary_refs.contains(&r.id))
        .ok_or_else(|| invalid("installed recipe has no boundary"))?;
    if inventory.archive().entry_bytes(&boundary.profile.file.path)
        != Some(configuration.profile_bytes.as_slice())
    {
        return Err(unsupported(
            "profile bytes are outside the installed 0.1.1 values recipe",
        ));
    }
    let actual = query.checked_file(&configuration.sdk)?;
    if inventory
        .archive()
        .entry_bytes(&configuration.sdk.file.path)
        != Some(actual.as_slice())
    {
        return Err(unsupported(
            "SDK bytes are outside the installed values recipe",
        ));
    }
    Ok(selected)
}
fn host_checks(selected: &SelectedTemplate, level: VerifyLevel) -> Vec<Check> {
    let observed = observe_initialization_host();
    let build = if level != VerifyLevel::Structure {
        observe_initialization_system_build().map(Some)
    } else {
        Ok(None)
    };
    let result = (|| -> Result<Fact> {
        let host = observed?;
        let build = build?;
        let mut sets = vec![&selected.profile().requirements.structure];
        if level != VerifyLevel::Structure {
            sets.push(&selected.profile().requirements.build);
        }
        if level == VerifyLevel::Run {
            sets.push(&selected.profile().requirements.run);
        }
        for set in sets {
            if !set.hosts.iter().any(|h| {
                h.os == host.os && h.arch == host.arch && h.versions.contains(host.version)
            }) || !set.ide_entries.is_empty()
            {
                return Err(invalid(
                    "observed host/IDE is outside the fixed selected requirements",
                ));
            }
        }
        Ok(Fact::Host {
            os: host.os,
            arch: host.arch,
            version: Some(host.version.to_string()),
            build,
        })
    })();
    let mut values = vec![match &result {
        Ok(fact) => Check::passed(
            "host",
            CheckKind::Host,
            "actual host requirements",
            None,
            fact.clone(),
        ),
        Err(error) => Check::failed("host", CheckKind::Host, "actual host requirements", error),
    }];
    if level == VerifyLevel::Run {
        values.push(match result {
            Ok(fact) => Check::passed(
                "run-host",
                CheckKind::Host,
                "host-run preconditions only; ordinary C was not executed",
                None,
                fact,
            ),
            Err(error) => Check::failed(
                "run-host",
                CheckKind::Host,
                "host-run preconditions",
                &error,
            ),
        });
    }
    values
}
fn check_requirements(
    selected: &SelectedTemplate,
    level: VerifyLevel,
    tools: &[ToolObservation],
) -> Result<()> {
    let requirements = &selected.profile().requirements;
    let mut sets = vec![
        requirements.structure.tools.as_slice(),
        requirements.build.tools.as_slice(),
    ];
    if level == VerifyLevel::Run {
        sets.push(requirements.run.tools.as_slice());
    }
    for requirement in
        madb_plan::project::merge_tool_requirements(&sets).map_err(|e| invalid(e.to_string()))?
    {
        if !tools.iter().any(|v| {
            v.id == requirement.id
                && v.version == requirement.version
                && v.sha256 == requirement.sha256
        }) {
            return Err(invalid(
                "required tool tuple differs from the compiled installation record",
            ));
        }
    }
    Ok(())
}
fn fill_blocked(report: &mut Report, level: VerifyLevel) {
    let mut blocker = if report
        .checks
        .iter()
        .any(|c| c.id.as_str() == "context.configuration" && c.status != Status::Passed)
    {
        "context.configuration"
    } else if report
        .checks
        .iter()
        .any(|c| c.id.as_str() == "recipe" && c.status != Status::Passed)
    {
        "recipe"
    } else {
        "sdk"
    };
    if !report.checks.iter().any(|c| c.id.as_str() == "recipe") {
        report.record(Check::blocked("recipe", CheckKind::Recipe, blocker));
        blocker = "recipe";
    }
    for name in required_checks(level) {
        if !report.checks.iter().any(|c| c.id.as_str() == name) {
            let kind = if name.starts_with("tool.") {
                CheckKind::Tool
            } else if name.starts_with("resource.") {
                CheckKind::Resource
            } else if name == "sdk" {
                CheckKind::Sdk
            } else {
                CheckKind::Host
            };
            report.record(Check::blocked(&name, kind, blocker));
        }
    }
}
fn invalid(message: impl Into<String>) -> QueryError {
    QueryError::new(ErrorKind::InvalidInput, "PLAN001", "doctor recipe", message)
}
fn id(value: &str) -> Id {
    Id::new(value).expect("static or compiler-provided identity")
}

fn unsupported(message: &str) -> QueryError {
    QueryError::new(
        ErrorKind::InvalidInput,
        "PLAN001",
        "doctor unsupported recipe",
        message,
    )
}
