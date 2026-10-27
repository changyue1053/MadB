use super::*;
use crate::initialization_source::FrozenInitializationTool;
use madb_app::transaction::CommitControl;
use rustix::fs::{FlockOperation, RenameFlags, flock, renameat_with};

impl LocalProjectBuild {
    pub(super) fn install_running(
        &mut self,
        d: &Descriptor,
        control: &CommitControl,
    ) -> InstallationObservation {
        let source = (|| -> Result<(FrozenInitializationTool, Vec<u8>)> {
            self.require_descriptor(d)?;
            self.verify_sources()?;
            let image = FrozenInitializationTool::capture_project(env!("CARGO_PKG_VERSION"))?;
            let revision = option_env!("MADB_SOURCE_REVISION").ok_or_else(|| {
                host(
                    "tool.source",
                    "running tool lacks an exact compiled source revision",
                )
            })?;
            if image.identity().id != d.tool.id
                || image.identity().version != d.tool.version
                || image.identity().sha256 != d.tool.sha256
                || image.image_size() != d.tool.size
                || revision != d.tool.source_revision
            {
                return Err(conflict(
                    "tool",
                    "running image and source identity differ from the descriptor",
                ));
            }
            let parent = image
                .image_path()
                .parent()
                .ok_or_else(|| invalid("tool", "image has no parent"))?;
            let name = image
                .image_path()
                .file_name()
                .and_then(|v| v.to_str())
                .ok_or_else(|| invalid("tool", "image name is not UTF-8"))?;
            let bytes = fs::read(&fs::absolute(parent)?, name, 256 * 1024 * 1024)?.0;
            image.revalidate()?;
            Ok((image, bytes))
        })();
        match source {
            Ok((image, bytes)) => self.install_checked(d, &bytes, control, || image.revalidate()),
            Err(error) => InstallationObservation {
                installed: None,
                error: Some(error),
            },
        }
    }
    // Private to this implementation (and unit fault tests); public installation
    // obtains bytes only from the independently captured running image above.
    #[cfg(test)]
    pub(super) fn install_bytes(
        &mut self,
        d: &Descriptor,
        bytes: &[u8],
        control: &CommitControl,
    ) -> InstallationObservation {
        self.install_checked(d, bytes, control, || Ok(()))
    }
    fn install_checked(
        &mut self,
        d: &Descriptor,
        bytes: &[u8],
        control: &CommitControl,
        revalidate: impl Fn() -> Result<()>,
    ) -> InstallationObservation {
        let mut installed = None;
        let result = (|| -> Result<()> {
            self.require_descriptor(d)?;
            let mut permit = control.begin()?;
            permit.reject_if_cancelled()?;
            self.verify_sources()?;
            if bytes.len() as u64 != d.tool.size || Digest::of_bytes(bytes) != d.tool.sha256 {
                return Err(conflict("tool", "installer bytes differ"));
            }
            let wrapper = fs::ensure_directories(&self.directory, ".madb/wrapper")?;
            flock(&wrapper, FlockOperation::LockExclusive).map_err(|e| host("wrapper lock", e))?;
            for name in fs::names(&wrapper, 16384)? {
                if name != "madb" && !name.starts_with(".install-") {
                    return Err(conflict(
                        name,
                        "unknown wrapper entry; no ownership assumed",
                    ));
                }
                fs::regular(&wrapper, &name)?;
            }
            let accepted = InstalledTool {
                path: AbsolutePath::new(&format!("{}/.madb/wrapper/madb", self.root.as_str()))
                    .map_err(|e| invalid("tool", e.to_string()))?,
                sha256: d.tool.sha256.clone(),
                size: d.tool.size,
            };
            let exists = fs::exists(&wrapper, "madb")?;
            let temporary = if exists {
                let (actual, _) = fs::read(&wrapper, "madb", 256 * 1024 * 1024)?;
                use std::os::unix::fs::PermissionsExt;
                if actual != bytes
                    || fs::regular(&wrapper, "madb")?
                        .metadata()
                        .map_err(|e| host("tool", e))?
                        .permissions()
                        .mode()
                        & 0o111
                        == 0
                {
                    return Err(conflict(
                        "tool",
                        "existing mirror differs; preserve or move it before retry",
                    ));
                }
                None
            } else {
                let temporary = format!(".install-{}", fs::nonce());
                fs::write_new(&wrapper, &temporary, bytes, 0o500)?;
                if fs::read(&wrapper, &temporary, 256 * 1024 * 1024)?.0 != bytes {
                    return Err(conflict("tool", "new mirror bytes differ"));
                }
                Some(temporary)
            };
            self.verify_sources()?;
            if !fs::same_directory(&wrapper, &fs::directory(&self.directory, ".madb/wrapper")?)? {
                return Err(conflict("wrapper", "mirror directory was replaced"));
            }
            if exists && fs::read(&wrapper, "madb", 256 * 1024 * 1024)?.0 != bytes {
                return Err(conflict(
                    "tool",
                    "existing mirror changed before acceptance",
                ));
            }
            revalidate()?;
            let name = temporary.as_deref().unwrap_or("madb");
            let (verified, stamp) = fs::read(&wrapper, name, 256 * 1024 * 1024)?;
            if verified != bytes {
                return Err(conflict("tool", "mirror changed before final acceptance"));
            }
            let temporary_stamp = temporary.as_ref().map(|_| stamp);
            let acceptance = permit.publish(|| {
                if let Some(name) = &temporary {
                    renameat_with(
                        &wrapper,
                        name.as_str(),
                        &wrapper,
                        "madb",
                        RenameFlags::NOREPLACE,
                    )
                    .map_err(|e| conflict("tool installation", e))?;
                }
                #[cfg(test)]
                if matches!(self.fault, Some(super::publish::Fault::InstallRenameError)) {
                    return Err(host(
                        "tool publication",
                        "injected post-rename syscall error",
                    ));
                }
                Ok(())
            });
            if let Err(error) = acceptance {
                // A failed no-replace syscall never permits overwriting. Retain
                // the installation fact only when the accepted path's bytes can
                // actually be established after the failure under this lock.
                if !control.rejected_by_cancellation()
                    && temporary_stamp.is_some()
                    && fs::regular(&wrapper, "madb")
                        .and_then(|f| fs::stamp(&f))
                        .ok()
                        .zip(temporary_stamp)
                        .is_some_and(|(a, b)| a.same_inode(&b))
                    && mirror_matches(&wrapper, bytes).unwrap_or(false)
                {
                    installed = Some(accepted);
                }
                return Err(error);
            }
            // The final gate has accepted the verified bytes. A later read,
            // evidence, or durability error must retain that completed effect.
            installed = Some(accepted);
            #[cfg(test)]
            if matches!(
                self.fault,
                Some(super::publish::Fault::InstallAfterAcceptRead)
            ) {
                return Err(host(
                    "tool verification",
                    "injected post-acceptance read failure",
                ));
            }
            if !mirror_matches(&wrapper, bytes)? {
                return Err(conflict(
                    "tool",
                    "accepted mirror bytes cannot be established; inspect the local mirror before retry",
                ));
            }
            revalidate()?;
            #[cfg(test)]
            if matches!(self.fault, Some(super::publish::Fault::InstallTail)) {
                return Err(host(
                    "tool durability",
                    "injected post-installation failure",
                ));
            }
            wrapper.sync_all().map_err(|e| host("tool durability", e))?;
            Ok(())
        })();
        InstallationObservation {
            installed,
            error: result.err(),
        }
    }
}

fn mirror_matches(wrapper: &File, bytes: &[u8]) -> Result<bool> {
    use std::os::unix::fs::PermissionsExt;
    Ok(fs::read(wrapper, "madb", 256 * 1024 * 1024)?.0 == bytes
        && fs::regular(wrapper, "madb")?
            .metadata()
            .map_err(|e| host("tool", e))?
            .permissions()
            .mode()
            & 0o111
            != 0)
}
