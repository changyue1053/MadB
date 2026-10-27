use super::*;
use madb_app::transaction::CommitControl;
use rustix::fs::{FlockOperation, flock, renameat};
use serde_json::json;

// Lock order: the .madb directory lock only establishes the closed owner/lock
// pair and is dropped before waiting for this commit lock. CommitControl's
// mutex surrounds only the atomic replacement, never either OS-lock wait,
// source/output IO, or an external process.
pub(super) struct State {
    pub directory: File,
    pub lock: File,
}
#[cfg(test)]
#[derive(Clone, Copy)]
pub(super) enum Fault {
    CancelBeforePublish,
    AfterPublish,
    UnknownAfterPublish,
    InstallTail,
    InstallRenameError,
    InstallAfterAcceptRead,
    AttemptAfterCreate,
}

impl LocalProjectBuild {
    pub(super) fn open_state(&self, create: bool) -> Result<Option<State>> {
        self.verify_root()?;
        let madb = fs::directory(&self.directory, ".madb")?;
        // The directory lock serializes first ownership without placing a lock
        // file in unknown contents. It is dropped before any commit lock wait.
        flock(&madb, FlockOperation::LockExclusive).map_err(|e| host("state initialization", e))?;
        let directory = if fs::exists(&madb, "project-state")? {
            fs::directory(&madb, "project-state")?
        } else if create {
            fs::create_directory(&madb, "project-state")?
        } else {
            return Ok(None);
        };
        let names = fs::names(&directory, 16384)?;
        let expected = self.owner_bytes()?;
        if names.is_empty() {
            if !create {
                return Ok(None);
            }
            fs::write_new(&directory, ".owner.json", &expected, 0o400)?;
            fs::write_new(&directory, ".commit-lock", b"", 0o600)?;
        } else {
            if fs::read(&directory, ".owner.json", MAX_WIRE_BYTES)?.0 != expected {
                return Err(conflict("state", "unrecognized project-state owner"));
            }
            for name in names {
                if !matches!(
                    name.as_str(),
                    ".owner.json" | ".commit-lock" | "current.json"
                ) && !name.starts_with(".index-")
                {
                    return Err(conflict(
                        name,
                        "unknown project-state entry; refusing adoption or cleanup",
                    ));
                }
                fs::regular(&directory, &name)?;
            }
        }
        let lock = fs::regular(&directory, ".commit-lock")?;
        Ok(Some(State { directory, lock }))
    }
    fn owner_bytes(&self) -> Result<Vec<u8>> {
        let origin = self
            .files
            .get(&logical(ORIGIN)?)
            .ok_or_else(|| conflict("state", "origin prerequisite is absent"))?;
        records::encode(
            &json!({"kind":"madb-project-state","version":1,"recipe":RECIPE,"origin_sha256":Digest::of_bytes(&origin.0).as_str()}),
        )
    }
    pub(super) fn read_baseline(&mut self) -> Result<Option<records::Index>> {
        let Some(state) = self.open_state(false)? else {
            return Ok(None);
        };
        flock(&state.lock, FlockOperation::LockShared).map_err(|e| host("state lock", e))?;
        let index = self.current(&state)?;
        if let Some(i) = &index {
            self.previous(i)?;
            let (bytes, stamp) = fs::read(&state.directory, "current.json", MAX_WIRE_BYTES)?;
            if records::index(&bytes)? != *i {
                return Err(conflict(
                    "current",
                    "current changed during baseline capture",
                ));
            }
            self.baseline_stamp = Some(stamp);
        } else {
            self.baseline_stamp = None;
        }
        Ok(index)
    }
    fn current(&self, state: &State) -> Result<Option<records::Index>> {
        self.check_state(state)?;
        if !fs::exists(&state.directory, "current.json")? {
            return Ok(None);
        }
        Ok(Some(records::index(
            &fs::read(&state.directory, "current.json", MAX_WIRE_BYTES)?.0,
        )?))
    }
    fn check_state(&self, state: &State) -> Result<()> {
        self.verify_root()?;
        let now = fs::directory(&self.directory, ".madb/project-state")?;
        if !fs::same_directory(&now, &state.directory)?
            || fs::stamp(&fs::regular(&now, ".commit-lock")?)? != fs::stamp(&state.lock)?
            || fs::read(&now, ".owner.json", MAX_WIRE_BYTES)?.0 != self.owner_bytes()?
        {
            return Err(conflict(
                "state",
                "project-state directory, lock, or owner was replaced",
            ));
        }
        Ok(())
    }
    pub(super) fn publish_output(
        &mut self,
        attempt: &BuildAttempt,
        output: &ProjectOutput,
        control: &CommitControl,
    ) -> PublicationObservation {
        let mut replaced = false;
        let result = (|| -> Result<()> {
            let mut permit = control.begin()?;
            permit.reject_if_cancelled()?;
            self.check_outputs(attempt, output)?;
            let owned = self.owned_attempt(attempt)?;
            let bytes = records::output_bytes(output)?;
            fs::write_new(&owned.directory, "project-output.json", &bytes, 0o400)?;
            let index = records::Index {
                generation: self
                    .baseline
                    .as_ref()
                    .map_or(0, |i| i.generation)
                    .checked_add(1)
                    .filter(|n| *n <= 9_007_199_254_740_991)
                    .ok_or_else(|| conflict("generation", "generation exhausted"))?,
                attempt: owned.name.clone(),
                record: FileIdentity {
                    path: logical("project-output.json")?,
                    sha256: Digest::of_bytes(&bytes),
                    size: bytes.len() as u64,
                },
                identity: output.identity.clone(),
            };
            let state = self
                .open_state(false)?
                .ok_or_else(|| conflict("state", "owned state disappeared"))?;
            flock(&state.lock, FlockOperation::LockExclusive).map_err(|e| host("state lock", e))?;
            let current = self.current(&state)?;
            let stamp = if current.is_some() {
                Some(fs::stamp(&fs::regular(&state.directory, "current.json")?)?)
            } else {
                None
            };
            if current != self.baseline || stamp != self.baseline_stamp {
                return Err(conflict(
                    "current",
                    "another build published; retain this attempt and start a new build",
                ));
            }
            if let Some(old) = &current {
                self.previous(old)?;
            }
            let temporary = format!(".index-{}", fs::nonce());
            fs::write_new(
                &state.directory,
                &temporary,
                &records::encode(&records::index_value(&index))?,
                0o600,
            )?;
            self.verify_sources()?;
            self.check_outputs(attempt, output)?;
            self.check_state(&state)?;
            if super::attempt::verify_file(&owned.directory, &index.record, MAX_WIRE_BYTES)?
                != bytes
            {
                return Err(conflict(
                    "project-output.json",
                    "immutable output record changed before publication",
                ));
            }
            let final_current = self.current(&state)?;
            let final_stamp = if final_current.is_some() {
                Some(fs::stamp(&fs::regular(&state.directory, "current.json")?)?)
            } else {
                None
            };
            if final_current != self.baseline || final_stamp != self.baseline_stamp {
                return Err(conflict(
                    "current",
                    "current changed during final publication checks",
                ));
            }
            #[cfg(test)]
            if matches!(self.fault, Some(Fault::CancelBeforePublish)) {
                control.cancel();
            }
            let accepted = permit.publish(|| {
                renameat(
                    &state.directory,
                    temporary.as_str(),
                    &state.directory,
                    "current.json",
                )
                .map_err(|e| host("current replacement", e))?;
                #[cfg(test)]
                if matches!(self.fault, Some(Fault::UnknownAfterPublish)) {
                    return Err(host(
                        "current replacement",
                        "injected indeterminate syscall result",
                    ));
                }
                Ok(())
            });
            if let Err(error) = accepted {
                // Replacement failure may be ambiguous; inspect while retaining
                // the same lock instead of claiming a rollback.
                let inspection = self.current(&state);
                #[cfg(test)]
                let inspection = if matches!(self.fault, Some(Fault::UnknownAfterPublish)) {
                    Err(host(
                        "current inspection",
                        "injected unavailable observation",
                    ))
                } else {
                    inspection
                };
                match inspection {
                    Ok(Some(actual)) if actual == index => {
                        replaced = true;
                        return Err(error);
                    }
                    Ok(actual) if actual == self.baseline => return Err(error),
                    _ => {
                        return Err(QueryError::new(
                            ErrorKind::Host,
                            "STORE008",
                            "current",
                            "replacement fact cannot be established; inspect current before retry",
                        ));
                    }
                }
            }
            replaced = true;
            #[cfg(test)]
            if matches!(self.fault, Some(Fault::AfterPublish)) {
                return Err(host(
                    "current durability",
                    "injected post-publication failure",
                ));
            }
            state
                .directory
                .sync_all()
                .map_err(|e| host("current durability", e))?;
            Ok(())
        })();
        match result {
            Ok(()) => PublicationObservation {
                published: Some(true),
                durability: Durability::Confirmed,
                error: None,
            },
            Err(error) => {
                let uncertain = error.findings.iter().any(|f| f.code == "STORE008");
                PublicationObservation {
                    published: if uncertain && !replaced {
                        None
                    } else {
                        Some(replaced)
                    },
                    durability: if replaced || uncertain {
                        Durability::Uncertain
                    } else {
                        Durability::NotApplicable
                    },
                    error: Some(error),
                }
            }
        }
    }
}
