//! Read-only facts from the same parsed record and verifiers as init. This type
//! cannot produce an InstalledValuesTools or expose a process request.
use super::*;
use crate::LocalWorkspace;
use madb_app::use_cases::doctor::{Check, CheckKind, Fact, State};

mod snapshot;
pub struct InstallationObserver {
    record: InstallationRecord,
}
pub struct InstallationFacts {
    checks: Vec<Check>,
    signatures: BTreeMap<String, Digest>,
}
impl InstallationFacts {
    pub fn checks(&self) -> &[Check] {
        &self.checks
    }
}
impl InstallationObserver {
    pub fn load() -> Result<Self> {
        Ok(Self {
            record: InstallationRecord::parse(RECORD)?,
        })
    }
    pub fn observations(&self) -> Vec<ToolObservation> {
        self.record
            .tools
            .values()
            .map(|v| v.observation.clone())
            .collect()
    }
    pub fn capture(&self) -> InstallationFacts {
        self.capture_between(|_| {})
    }
    // Private observation seam; production supplies no action. Tests use it to
    // replace an object exactly between its read and identity recheck.
    fn capture_between(&self, mut between: impl FnMut(&str)) -> InstallationFacts {
        let mut checks = Vec::new();
        let mut signatures = BTreeMap::new();
        for (name, tool) in &self.record.tools {
            let check_id = format!("tool.{name}");
            let before = snapshot::files(std::iter::once(tool.file.path.clone()));
            let observed = read_file(&tool.file);
            between(&check_id);
            let snapshot = snapshot::files(std::iter::once(tool.file.path.clone()));
            signatures.insert(check_id.clone(), snapshot.signature.clone());
            let mut check = match &observed {
                Ok(Some(bytes))
                    if bytes.len() as u64 == tool.file.size
                        && madb_app::Digest::of_bytes(bytes) == tool.file.digest =>
                {
                    Check::passed(
                        &check_id,
                        CheckKind::Tool,
                        tool.file.path.display().to_string(),
                        None,
                        Fact::Tool(tool.observation.clone()),
                    )
                }
                _ => {
                    let error = match &observed {
                        Err(e) => madb_app::QueryError {
                            kind: e.kind,
                            findings: e.findings.clone(),
                        },
                        _ => match tool.verify() {
                            Err(e) => e,
                            Ok(()) => conflict("tool", "tool changed between bounded observations"),
                        },
                    };
                    let mut check = Check::failed(
                        &check_id,
                        CheckKind::Tool,
                        tool.file.path.display().to_string(),
                        &error,
                    );
                    check.observed = match observed {
                        Ok(Some(ref bytes)) => Some(Fact::bytes(bytes)),
                        Ok(None) => Some(Fact::State(State::Missing)),
                        Err(_) => None,
                    };
                    check
                }
            };
            if let Some(error) = before.error.or(snapshot.error) {
                let mut failed =
                    Check::failed(&check_id, CheckKind::Tool, check.subject.clone(), &error);
                failed.observed = check.observed;
                check = failed;
            }
            if before.signature != snapshot.signature {
                let mut changed = Check::failed(
                    &check_id,
                    CheckKind::Tool,
                    check.subject.clone(),
                    &conflict(
                        "doctor tool",
                        "file identity changed around bounded tool read",
                    ),
                );
                changed.observed = check.observed;
                check = changed;
            }
            check.expected = Some(Fact::Tool(tool.observation.clone()));
            checks.push(check);
        }
        let snapshot = snapshot::files(self.record.sdk.iter().map(|file| file.path.clone()));
        let mut sdk_result = snapshot.error.map_or_else(
            || {
                self.record
                    .sdk
                    .iter()
                    .try_for_each(LockedInitializationFile::verify)
            },
            Err,
        );
        between("sdk");
        let after = snapshot::files(self.record.sdk.iter().map(|file| file.path.clone()));
        if snapshot.signature != after.signature {
            sdk_result = Err(conflict(
                "doctor SDK",
                "input identity changed around SDK observation",
            ));
        }
        signatures.insert("sdk".into(), after.signature);
        checks.push(group(
            "sdk",
            CheckKind::Sdk,
            "complete fixed SDK input inventory",
            NATIVE,
            sdk_result,
        ));
        for (id, subject, value, verify) in [
            (
                "resource.installation",
                "fixed installation tree, receipt and aliases",
                &self.record.installation,
                verify_installation as fn(&Value) -> Result<()>,
            ),
            (
                "resource.python",
                "fixed isolated Python runtime and library aliases",
                &self.record.python_runtime,
                verify_python,
            ),
            (
                "resource.compiler",
                "finite fixed compiler loader chain and system build",
                &self.record.compiler_runtime,
                verify_compiler,
            ),
        ] {
            let before = snapshot::record(value);
            let mut result = before.error.map_or_else(|| verify(value), Err);
            between(id);
            let after = snapshot::record(value);
            if before.signature != after.signature {
                result = Err(conflict(
                    "doctor resources",
                    "resource identity changed around validation",
                ));
            }
            signatures.insert(id.into(), after.signature);
            checks.push(group(id, CheckKind::Resource, subject, RECORD, result));
        }
        InstallationFacts { checks, signatures }
    }
    pub fn reobserve(&self, initial: &InstallationFacts) -> Vec<Check> {
        let mut final_value = self.capture();
        for check in &mut final_value.checks {
            if initial.signatures.get(check.id.as_str())
                != final_value.signatures.get(check.id.as_str())
            {
                let mut changed = Check::failed(
                    check.id.as_str(),
                    check.kind,
                    check.subject.clone(),
                    &conflict(
                        "doctor resources",
                        "actual file bytes, directory identity or alias changed during observation",
                    ),
                );
                changed.expected = check.expected.clone();
                changed.observed = check.observed.clone();
                *check = changed;
            }
        }
        final_value.checks
    }
}
fn group(id: &str, kind: CheckKind, subject: &str, record: &[u8], result: Result<()>) -> Check {
    let mut check = match result {
        Ok(()) => Check::passed(id, kind, subject, None, Fact::State(State::Checked)),
        Err(error) => Check::failed(
            id,
            kind,
            format!("{subject}; inspection stopped at first error in this group"),
            &error,
        ),
    };
    check.expected = Some(Fact::bytes(record));
    check
}
fn read_file(file: &LockedInitializationFile) -> Result<Option<Vec<u8>>> {
    if file.size > 256 * 1024 * 1024 {
        return Err(invalid("tool", "locked image exceeds 256 MiB budget"));
    }
    let path = super::super::absolute_relative(&file.path)?;
    LocalWorkspace::open(Path::new("/"))?.read_with_policy(&path, 256 * 1024 * 1024, true)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use madb_app::{
        ErrorKind,
        use_cases::doctor::{Report, reconcile},
    };
    use std::{
        fs,
        os::unix::fs::{PermissionsExt, symlink},
        sync::atomic::{AtomicU64, Ordering},
    };
    fn root() -> PathBuf {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = fs::canonicalize(std::env::temp_dir())
            .unwrap()
            .join(format!(
                "madb-doctor-install-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
        fs::create_dir(&path).unwrap();
        path
    }
    fn observer(root: &Path) -> InstallationObserver {
        let mut record = InstallationRecord::parse(RECORD).unwrap();
        record
            .tools
            .retain(|name, _| ["rustc", "ninja"].contains(&name.as_str()));
        for (name, tool) in &mut record.tools {
            tool.file = LockedInitializationFile {
                path: root.join(name),
                digest: Digest::of_bytes(b"expected"),
                size: 8,
            };
            tool.observation.sha256 = tool.file.digest.clone();
        }
        record.sdk = vec![LockedInitializationFile {
            path: root.join("sdk"),
            digest: Digest::of_bytes(b"sdk"),
            size: 3,
        }];
        record.installation =
            serde_json::json!({"root":root.join("missing-installation"),"entries":[]});
        record.python_runtime = serde_json::json!({});
        record.compiler_runtime = serde_json::json!({});
        InstallationObserver { record }
    }
    #[test]
    fn independent_missing_wrong_bytes_and_sdk_missing_keep_real_categories() {
        let root = root();
        fs::write(root.join("ninja"), b"bad").unwrap();
        let observer = observer(&root);
        assert!(observer.record.verify().is_err());
        let facts = observer.capture();
        for (id, kind) in [
            ("tool.rustc", ErrorKind::Host),
            ("tool.ninja", ErrorKind::InvalidInput),
            ("sdk", ErrorKind::Host),
        ] {
            let row = facts.checks().iter().find(|c| c.id.as_str() == id).unwrap();
            assert_eq!(row.error_kind, Some(kind));
            assert!(!matches!(row.observed, Some(Fact::Tool(_))));
        }
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn bad_bytes_changing_to_different_bad_bytes_and_alias_changes_are_detected() {
        let root = root();
        fs::write(root.join("ninja"), b"bad-one").unwrap();
        let observer = observer(&root);
        let initial = observer.capture();
        fs::write(root.join("ninja"), b"bad-two").unwrap();
        let mut report = Report::default();
        reconcile(
            &mut report,
            initial.checks.clone(),
            observer.reobserve(&initial),
        )
        .unwrap();
        assert_eq!(
            report
                .checks
                .iter()
                .find(|c| c.id.as_str() == "tool.ninja")
                .unwrap()
                .error_kind,
            Some(ErrorKind::Conflict)
        );
        symlink("ninja", root.join("alias")).unwrap();
        let value = serde_json::json!({"library_links":[{"path":root.join("alias")}]});
        let first = snapshot::record(&value);
        fs::remove_file(root.join("alias")).unwrap();
        symlink("other", root.join("alias")).unwrap();
        assert_ne!(first.signature, snapshot::record(&value).signature);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn sdk_permission_failure_is_host_without_weakening_execution_verifier() {
        let root = root();
        fs::write(root.join("sdk"), b"sdk").unwrap();
        fs::set_permissions(root.join("sdk"), fs::Permissions::from_mode(0o0)).unwrap();
        let facts = observer(&root).capture();
        fs::set_permissions(root.join("sdk"), fs::Permissions::from_mode(0o600)).unwrap();
        assert_eq!(
            facts
                .checks()
                .iter()
                .find(|c| c.id.as_str() == "sdk")
                .unwrap()
                .error_kind,
            Some(ErrorKind::Host)
        );
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn same_bytes_replacement_between_read_and_identity_check_is_conflict() {
        for target in ["tool.ninja", "sdk"] {
            let root = root();
            fs::write(root.join("ninja"), b"expected").unwrap();
            fs::write(root.join("sdk"), b"sdk").unwrap();
            let observer = observer(&root);
            let facts = observer.capture_between(|id| {
                if id == target {
                    let name = if id == "sdk" { "sdk" } else { "ninja" };
                    let bytes = fs::read(root.join(name)).unwrap();
                    fs::rename(root.join(name), root.join("previous")).unwrap();
                    fs::write(root.join(name), bytes).unwrap();
                }
            });
            assert_eq!(
                facts
                    .checks()
                    .iter()
                    .find(|c| c.id.as_str() == target)
                    .unwrap()
                    .error_kind,
                Some(ErrorKind::Conflict)
            );
            fs::remove_dir_all(root).unwrap();
        }
    }
}
