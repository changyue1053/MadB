//! Host observations for installed initialization. No process is started by
//! capture, revalidation, output normalization or operating-system observation.
use madb_app::use_cases::initialize::{AbsolutePath, ComponentIdentity, Version};
use madb_app::{Digest, ErrorKind, Id, QueryError, Result};
use std::path::{Path, PathBuf};

#[cfg(target_os = "macos")]
mod mapped_macos;

const IMAGE_LIMIT: usize = 128 * 1024 * 1024;

pub struct FrozenInitializationTool {
    identity: ComponentIdentity,
    path: PathBuf,
    file: std::fs::File,
    stamp: Stamp,
    running_image: bool,
}
impl FrozenInitializationTool {
    pub fn capture(version: &str) -> Result<Self> {
        let path = std::env::current_exe().map_err(|e| host("tool", e))?;
        let mut captured = Self::capture_path(path, version)?;
        require_running_image(&captured.stamp)?;
        captured.running_image = true;
        captured.revalidate()?;
        Ok(captured)
    }
    fn capture_path(path: PathBuf, version: &str) -> Result<Self> {
        let file = open_regular(&path)?;
        let (bytes, stamp) = read_stable(&file, IMAGE_LIMIT)?;
        Ok(Self {
            identity: ComponentIdentity {
                id: Id::new("org.madb/cli").expect("fixed tool identity"),
                version: version.to_owned(),
                sha256: Digest::of_bytes(&bytes),
            },
            path,
            file,
            stamp,
            running_image: false,
        })
    }
    pub fn identity(&self) -> &ComponentIdentity {
        &self.identity
    }
    /// The held inode and its original pathname must still identify the same
    /// regular image. Replacement, in-place writes and parent redirection fail.
    pub fn revalidate(&self) -> Result<()> {
        let check = || -> Result<()> {
            if self.running_image {
                require_running_image(&self.stamp)?;
            }
            for file in [&self.file, &open_regular(&self.path)?] {
                let (bytes, stamp) = read_stable(file, IMAGE_LIMIT)?;
                if stamp != self.stamp || Digest::of_bytes(&bytes) != self.identity.sha256 {
                    return Err(conflict("installed tool image changed"));
                }
            }
            if self.running_image {
                require_running_image(&self.stamp)?;
            }
            Ok(())
        };
        check().map_err(|_| conflict("installed tool image cannot be revalidated"))
    }
}

fn require_running_image(stamp: &Stamp) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        mapped_macos::require_running_image(stamp)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = stamp;
        Err(QueryError::new(
            ErrorKind::Host,
            "HOST004",
            "tool.mapping",
            "running executable identity observation is not implemented on this OS",
        ))
    }
}

pub struct InitializationHost {
    pub os: Id,
    pub arch: Id,
    pub version: Version,
}

pub fn normalize_output(path: &Path) -> Result<AbsolutePath> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|e| host("output", e))?
            .join(path)
    };
    let text = absolute
        .to_str()
        .ok_or_else(|| invalid("output is not UTF-8"))?;
    // This is lexical only: the creator subsequently walks and retains all
    // real parent directory descriptors, without following symbolic links.
    AbsolutePath::new(text)
}

pub fn observe_initialization_host() -> Result<InitializationHost> {
    #[cfg(target_os = "macos")]
    {
        // Apple's root-owned system version file is local observation, not a
        // template input. Do not invoke sw_vers, including during dry-run.
        let file = open_regular(Path::new(
            "/System/Library/CoreServices/SystemVersion.plist",
        ))?;
        let (bytes, _) = read_stable(&file, 64 * 1024)?;
        Ok(InitializationHost {
            os: Id::new(std::env::consts::OS).expect("fixed OS identifier"),
            arch: Id::new(std::env::consts::ARCH).expect("fixed architecture identifier"),
            version: system_product_version(&bytes)?,
        })
    }
    #[cfg(not(target_os = "macos"))]
    Err(QueryError::new(
        ErrorKind::Host,
        "HOST004",
        "host.version",
        "initialization host version observation is not implemented on this OS",
    ))
}

// Only the fixed flat XML system-version file is accepted. This parser does
// not expand entities, follow a DTD, accept nested dicts or parse arbitrary XML.
#[cfg(any(target_os = "macos", test))]
fn system_product_version(bytes: &[u8]) -> Result<Version> {
    let source = std::str::from_utf8(bytes).map_err(|_| invalid("system version is not UTF-8"))?;
    let prefix = [
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>",
        "<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">",
        "<plist version=\"1.0\">",
        "<dict>",
    ];
    let mut dict = source;
    for token in prefix {
        dict = dict
            .trim_start()
            .strip_prefix(token)
            .ok_or_else(|| invalid("unexpected system-version XML envelope"))?;
    }
    let (mut dict, tail) = dict
        .split_once("</dict>")
        .ok_or_else(|| invalid("unclosed system dictionary"))?;
    if tail.trim() != "</plist>" || dict.contains("<dict") || source.matches("<plist ").count() != 1
    {
        return Err(invalid("unexpected system-version XML shape"));
    }
    let mut version = None;
    let mut keys = std::collections::BTreeSet::new();
    while !dict.trim().is_empty() {
        let key_start = dict
            .trim_start()
            .strip_prefix("<key>")
            .ok_or_else(|| invalid("expected system key"))?;
        let (key, remainder) = key_start
            .split_once("</key>")
            .ok_or_else(|| invalid("unclosed system key"))?;
        if !keys.insert(key) || key.contains(['<', '&']) {
            return Err(invalid("duplicate or escaped system key"));
        }
        let value_start = remainder
            .trim_start()
            .strip_prefix("<string>")
            .ok_or_else(|| invalid("expected system string"))?;
        let (value, remainder) = value_start
            .split_once("</string>")
            .ok_or_else(|| invalid("unclosed system string"))?;
        if value.contains(['<', '&']) {
            return Err(invalid("escaped system string"));
        }
        if key == "ProductVersion" {
            version = Some(
                Version::parse(value).map_err(|_| invalid("system version is not formal N.N.N"))?,
            );
        }
        dict = remainder;
    }
    version.ok_or_else(|| invalid("system ProductVersion is missing"))
}

#[cfg(unix)]
fn open_regular(path: &Path) -> Result<std::fs::File> {
    use rustix::fs::{Mode, OFlags, open, openat};
    use std::path::Component;
    let mut parts = path.components();
    if parts.next() != Some(Component::RootDir) {
        return Err(invalid("source path must be absolute"));
    }
    let base = OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK;
    let mut file = std::fs::File::from(
        open("/", base | OFlags::DIRECTORY, Mode::empty()).map_err(|e| host("source", e))?,
    );
    let mut parts = parts.peekable();
    if parts.peek().is_none() {
        return Err(invalid("source cannot be filesystem root"));
    }
    while let Some(part) = parts.next() {
        let Component::Normal(name) = part else {
            return Err(invalid("source path is not normalized"));
        };
        let flags = base
            | if parts.peek().is_some() {
                OFlags::DIRECTORY
            } else {
                OFlags::empty()
            };
        file = std::fs::File::from(
            openat(&file, name, flags, Mode::empty()).map_err(|e| host("source", e))?,
        );
    }
    if !file.metadata().map_err(|e| host("source", e))?.is_file() {
        return Err(invalid("source is not a regular file"));
    }
    Ok(file)
}
#[cfg(not(unix))]
fn open_regular(_path: &Path) -> Result<std::fs::File> {
    Err(QueryError::new(
        ErrorKind::Host,
        "HOST004",
        "source",
        "confined image observations require Unix",
    ))
}

#[derive(Eq, PartialEq)]
struct Stamp {
    size: u64,
    #[cfg(unix)]
    identity: (u64, u64, i64, i64, i64, i64, u32),
}
fn stamp(file: &std::fs::File) -> Result<Stamp> {
    let metadata = file.metadata().map_err(|e| host("source", e))?;
    if !metadata.is_file() {
        return Err(invalid("source is not a regular file"));
    }
    #[cfg(unix)]
    use std::os::unix::fs::MetadataExt;
    Ok(Stamp {
        size: metadata.len(),
        #[cfg(unix)]
        identity: (
            metadata.dev(),
            metadata.ino(),
            metadata.mtime(),
            metadata.mtime_nsec(),
            metadata.ctime(),
            metadata.ctime_nsec(),
            metadata.mode(),
        ),
    })
}
fn read_stable(file: &std::fs::File, limit: usize) -> Result<(Vec<u8>, Stamp)> {
    let before = stamp(file)?;
    if before.size > limit as u64 {
        return Err(invalid("source exceeds bounded observation limit"));
    }
    let mut bytes = vec![0; before.size as usize];
    #[cfg(unix)]
    {
        use std::os::unix::fs::FileExt;
        file.read_exact_at(&mut bytes, 0)
            .map_err(|e| host("source", e))?;
    }
    #[cfg(not(unix))]
    {
        use std::io::{Read, Seek, SeekFrom};
        let mut reader = file.try_clone().map_err(|e| host("source", e))?;
        reader
            .seek(SeekFrom::Start(0))
            .map_err(|e| host("source", e))?;
        reader
            .read_exact(&mut bytes)
            .map_err(|e| host("source", e))?;
    }
    if bytes.len() > limit || bytes.len() as u64 != before.size || stamp(file)? != before {
        return Err(conflict("source changed during observation"));
    }
    Ok((bytes, before))
}
fn invalid(message: &str) -> QueryError {
    QueryError::new(
        ErrorKind::InvalidInput,
        "HOST002",
        "initialization source",
        message,
    )
}
fn conflict(message: &str) -> QueryError {
    QueryError::new(
        ErrorKind::Conflict,
        "STORE002",
        "initialization source",
        message,
    )
}
fn host(subject: &str, error: impl std::fmt::Display) -> QueryError {
    QueryError::new(ErrorKind::Host, "HOST002", subject, error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn product_version_is_a_unique_formal_system_value() {
        let xml = |inner: &str| {
            format!(
                "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\"><dict>{inner}</dict></plist>"
            )
        };
        assert_eq!(
            system_product_version(
                xml("<key>ProductVersion</key><string>26.6.2</string>").as_bytes()
            )
            .unwrap()
            .to_string(),
            "26.6.2"
        );
        let valid = xml("<key>ProductVersion</key><string>26.6.2</string>");
        for malformed in [
            format!("junk{valid}"),
            valid.replacen("<plist version=\"1.0\">", "<plist version=\"2.0\">", 1),
            valid.replacen("<?xml version=\"1.0\" encoding=\"UTF-8\"?>", "", 1),
            valid.replacen(
                "http://www.apple.com/DTDs/PropertyList-1.0.dtd",
                "https://untrusted.invalid/other.dtd",
                1,
            ),
        ] {
            assert!(system_product_version(malformed.as_bytes()).is_err());
        }
        for inner in [
            "",
            "<key>ProductVersion</key><string>26.6</string>",
            "<key>ProductVersion</key><string>26.6.2-rc.1</string>",
            "<key>ProductVersion</key><string>26.6.2</string><key>ProductVersion</key><string>26.6.2</string>",
            "<key>ProductVersion</key><string>&version;</string>",
            "<dict></dict>",
        ] {
            assert!(
                system_product_version(xml(inner).as_bytes()).is_err(),
                "{inner}"
            );
        }
    }
    #[test]
    fn held_and_named_image_must_remain_identical() {
        let root = crate::locations::temporary_parent()
            .unwrap()
            .join(crate::locations::unique_run_name().as_str());
        std::fs::create_dir(&root).unwrap();
        let path = root.join("tool");
        std::fs::write(&path, b"original image").unwrap();
        let captured = FrozenInitializationTool::capture_path(path.clone(), "0.1.0").unwrap();
        captured.revalidate().unwrap();
        std::fs::write(root.join("replacement"), b"original image").unwrap();
        std::fs::rename(root.join("replacement"), &path).unwrap();
        assert_eq!(captured.revalidate().unwrap_err().kind, ErrorKind::Conflict);
        let captured = FrozenInitializationTool::capture_path(path.clone(), "0.1.0").unwrap();
        std::fs::write(&path, b"modified image").unwrap();
        assert_eq!(captured.revalidate().unwrap_err().kind, ErrorKind::Conflict);
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&path, root.join("alias")).unwrap();
            assert!(FrozenInitializationTool::capture_path(root.join("alias"), "0.1.0").is_err());
        }
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    #[cfg(target_os = "macos")]
    fn actual_running_image_is_observed_without_version_substitution() {
        let tool = FrozenInitializationTool::capture(env!("CARGO_PKG_VERSION")).unwrap();
        assert_eq!(tool.identity().version, env!("CARGO_PKG_VERSION"));
        tool.revalidate().unwrap();
    }
    #[test]
    #[cfg(target_os = "macos")]
    fn actual_host_observation_has_the_compiled_architecture() {
        let observed = observe_initialization_host().unwrap();
        assert_eq!(observed.os.as_str(), std::env::consts::OS);
        assert_eq!(observed.arch.as_str(), std::env::consts::ARCH);
    }
}
