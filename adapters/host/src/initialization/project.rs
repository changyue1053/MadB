use super::{
    InitializationEvent, InitializationPoint, failure, filesystem as fs, host, staging::Stage,
};
use madb_app::use_cases::initialize::{
    AbsolutePath, CommitNewPort, HostCommitObservation, InitialFile, PreparedProject,
};
use madb_app::{
    ErrorKind, Result,
    transaction::{CommitControl, CommitPermit},
};
use std::collections::BTreeSet;

/// One request's physical parent and single-use cancellation/publication gate.
/// The parent must exist and every component must have its exact spelling and
/// be a directory, never a symlink. No constructor creates the output root.
pub struct LocalProjectCreator {
    binding: fs::ParentBinding,
    control: CommitControl,
    cancellation_observed: bool,
}
impl LocalProjectCreator {
    pub fn open(root: &AbsolutePath, control: CommitControl) -> Result<Self> {
        Ok(Self {
            binding: fs::ParentBinding::open(root)?,
            control,
            cancellation_observed: false,
        })
    }
    /// True only when this invocation actually observed cancellation before
    /// creation or its publication permit vetoed the syscall. A concurrently
    /// set signal flag does not reclassify an unrelated filesystem failure.
    pub fn cancellation_observed(&self) -> bool {
        self.cancellation_observed
    }
    pub fn commit_new_with(
        &mut self,
        project: &PreparedProject,
        hook: impl FnMut(InitializationEvent<'_>) -> Result<()>,
    ) -> Result<HostCommitObservation> {
        if project.root() != &self.binding.root || project.request().projection().dry_run {
            return Err(failure(
                ErrorKind::InvalidInput,
                "commit",
                "project root differs from Host binding or request is dry-run",
            ));
        }
        let mut directories = BTreeSet::new();
        for module in project.plan().modules() {
            if let Some(path) = &module.directory {
                fs::add_parents(path.as_str(), &mut directories);
                directories.insert(path.as_str().to_owned());
            }
        }
        for file in project.files() {
            fs::add_parents(file.path().as_str(), &mut directories);
        }
        let executable_wrapper = project.request().selected().template().version
            == madb_app::use_cases::initialize::Version::parse("0.2.0").expect("fixed version")
            && project
                .request()
                .selected()
                .template()
                .task_roots
                .iter()
                .any(|t| t.recipe.as_str() == madb_app::use_cases::project_build::RECIPE);
        self.commit_files_mode(project.files(), directories, executable_wrapper, hook)
    }
    #[cfg(test)]
    fn commit_files(
        &mut self,
        files: &[InitialFile],
        directories: BTreeSet<String>,
        hook: impl FnMut(InitializationEvent<'_>) -> Result<()>,
    ) -> Result<HostCommitObservation> {
        self.commit_files_mode(files, directories, false, hook)
    }
    fn commit_files_mode(
        &mut self,
        files: &[InitialFile],
        directories: BTreeSet<String>,
        executable_wrapper: bool,
        mut hook: impl FnMut(InitializationEvent<'_>) -> Result<()>,
    ) -> Result<HostCommitObservation> {
        let permit = self.control.begin()?;
        check_cancelled(&permit, &mut self.cancellation_observed)?;
        self.binding.verify()?;
        let mut stage = Stage::create(&self.binding)?;
        let staged: Result<Vec<_>> = (|| {
            event(&mut hook, InitializationPoint::StageCreated, &stage)?;
            for directory in &directories {
                check_cancelled(&permit, &mut self.cancellation_observed)?;
                stage.directory(directory)?;
            }
            for file in files {
                check_cancelled(&permit, &mut self.cancellation_observed)?;
                if let Some(bytes) = file.bytes() {
                    if executable_wrapper && file.path().as_str() == "madbw" {
                        stage.write_project_wrapper(bytes)?;
                    } else {
                        stage.write(file.path().as_str(), bytes)?;
                    }
                    event(&mut hook, InitializationPoint::FileWritten, &stage)?;
                }
            }
            stage.sync()?;
            check_cancelled(&permit, &mut self.cancellation_observed)?;
            event(&mut hook, InitializationPoint::BeforePublish, &stage)?;
            self.binding.verify()?;
            stage.verify_name()?;
            let expected: Vec<_> = files.iter().filter_map(InitialFile::reference).collect();
            fs::verify_tree(&stage.directory, &expected, &directories)?;
            Ok(expected)
        })();
        let expected = match staged {
            Ok(expected) => expected,
            Err(mut error) => {
                if let Err(cleanup) = stage.discard() {
                    error.findings.extend(cleanup.findings);
                }
                return Err(error);
            }
        };
        let mut publication_attempted = false;
        let published = permit.publish(|| {
            publication_attempted = true;
            stage.publish(&self.binding)
        });
        if let Err(mut error) = published {
            // The typed cancellation gate, not a string/code heuristic, decides
            // the I01 cancellation category. Failed no-replace remains Conflict.
            if !publication_attempted {
                self.cancellation_observed = true;
                error = cancelled();
            }
            if let Err(cleanup) = stage.discard() {
                error.findings.extend(cleanup.findings);
            }
            return Err(error);
        }
        // From here every failure is an observation on a created project. Never
        // propagate Err through the port, retry publication, or delete this root.
        let observed = (|| {
            event(&mut hook, InitializationPoint::AfterPublish, &stage)?;
            self.binding
                .parent
                .sync_all()
                .map_err(|e| host("parent durability", e))?;
            stage.verify_published(&self.binding)?;
            let files = fs::verify_tree(&stage.directory, &expected, &directories)?;
            stage.verify_published(&self.binding)?;
            Ok(files)
        })();
        Ok(HostCommitObservation {
            root: self.binding.root.clone(),
            files: observed,
        })
    }
}
impl CommitNewPort for LocalProjectCreator {
    fn commit_new(&mut self, project: &PreparedProject) -> Result<HostCommitObservation> {
        self.commit_new_with(project, |_| Ok(()))
    }
}
fn event(
    hook: &mut impl FnMut(InitializationEvent<'_>) -> Result<()>,
    point: InitializationPoint,
    stage: &Stage,
) -> Result<()> {
    hook(InitializationEvent {
        point,
        staging: &stage.path,
    })
}
fn cancelled() -> madb_app::QueryError {
    failure(
        ErrorKind::CheckFailed,
        "commit",
        "initialization was cancelled before creation",
    )
}
fn check_cancelled(permit: &CommitPermit<'_>, observed: &mut bool) -> Result<()> {
    if permit.cancelled() {
        *observed = true;
        Err(cancelled())
    } else {
        Ok(())
    }
}

#[cfg(test)]
#[path = "project_tests.rs"]
mod tests;
