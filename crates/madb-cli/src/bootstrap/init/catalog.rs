//! A private installation record. Callers cannot load a catalog, package path,
//! URL, or executable override through the initialization command.
use madb_app::{Digest, ErrorKind, Id, LogicalPath, QueryError, Result};
use madb_plan::project::{ComponentIdentity, FileIdentity, FileReference, VALUES_RECIPE, Version};

pub(super) struct TrustedTemplateRecord {
    pub id: Id,
    pub version: Version,
    pub archive: FileIdentity,
    pub manifest: FileReference,
    pub source: BundledSource,
    pub recipes: Vec<ComponentIdentity>,
    bytes: &'static [u8],
}
pub(super) enum BundledSource {
    Bundled,
}

impl TrustedTemplateRecord {
    /// Only immutable bytes compiled into this tool can enter the bundled
    /// installation record. The executing image is observed by Host separately.
    pub fn bundled(
        tool: &ComponentIdentity,
        bytes: &'static [u8],
        version: Version,
        archive: FileIdentity,
        manifest: FileReference,
    ) -> Result<Self> {
        if manifest.schema.as_str() != "madb.project-template/1"
            || Digest::of_bytes(bytes) != archive.sha256
            || bytes.len() as u64 != archive.size
        {
            return Err(invalid(
                "bundled template differs from its fixed archive or manifest format",
            ));
        }
        Ok(Self {
            id: Id::new("org.madb.template/rust-c-sdk").expect("fixed template id"),
            version,
            archive,
            manifest,
            source: BundledSource::Bundled,
            recipes: vec![ComponentIdentity {
                id: Id::new(VALUES_RECIPE).expect("fixed recipe id"),
                version: tool.version.clone(),
                sha256: tool.sha256.clone(),
            }],
            bytes,
        })
    }
    pub fn select(&self, selector: Option<&str>) -> Result<()> {
        let fixed = format!("{}@{}", self.id, self.version);
        if selector.is_some_and(|value| value != self.id.as_str() && value != fixed) {
            return Err(invalid(
                "selector is not a template/version in this installation's fixed catalog",
            ));
        }
        Ok(())
    }
    pub fn bytes(&self) -> &'static [u8] {
        self.bytes
    }
    pub fn recipe(&self, id: &Id) -> Result<&ComponentIdentity> {
        self.recipes
            .iter()
            .find(|entry| &entry.id == id)
            .ok_or_else(|| invalid("selected recipe is not installed by this tool image"))
    }
}
pub(super) fn path(value: &str) -> LogicalPath {
    LogicalPath::new(value).expect("fixed installed logical path")
}
fn invalid(message: impl Into<String>) -> QueryError {
    QueryError::new(
        ErrorKind::InvalidInput,
        "PLAN001",
        "template catalog",
        message,
    )
}

/// Fixed package shipped in this producer. The build records these raw bytes;
/// no runtime filesystem path or remote catalog can replace the installation.
pub(super) fn installed(
    tool: &ComponentIdentity,
    selector: Option<&str>,
) -> Result<TrustedTemplateRecord> {
    if selector == Some("org.madb.template/rust-c-sdk@0.1.0") {
        return installed_legacy(tool);
    }
    let record = TrustedTemplateRecord::bundled(
        tool,
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../templates/projects/rust-c-sdk-values/versions/0.1.1/package.tar"
        )),
        Version::parse("0.1.1").expect("fixed template version"),
        FileIdentity {
            path: path("templates/projects/rust-c-sdk-values/versions/0.1.1/package.tar"),
            sha256: Digest::new(
                "sha256:ffd2687530f5510374d7da389cac9ac883691adb28ad11cf31a3a6fc297d4a93",
            )
            .expect("fixed archive digest"),
            size: 49_664,
        },
        FileReference {
            file: FileIdentity {
                path: path("meta/template.json"),
                sha256: Digest::new(
                    "sha256:47ee331592c12aca99cfdd5fbc80a55e1fb2109039dbad3a4b813eb84c94be7d",
                )
                .expect("fixed manifest digest"),
                size: 13_914,
            },
            schema: Id::new("madb.project-template/1").expect("fixed schema"),
        },
    )?;
    record.select(selector)?;
    Ok(record)
}
fn installed_legacy(tool: &ComponentIdentity) -> Result<TrustedTemplateRecord> {
    const ARCHIVE: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../templates/projects/rust-c-sdk-values/package.tar"
    ));
    TrustedTemplateRecord::bundled(
        tool,
        ARCHIVE,
        Version::parse("0.1.0").expect("legacy template version"),
        FileIdentity {
            path: path("templates/projects/rust-c-sdk-values/package.tar"),
            sha256: Digest::new(
                "sha256:97d0e9fd5747bd5ceea920574e75c8e17825e379037c94b68e878649a310a850",
            )
            .expect("fixed archive digest"),
            size: 43_520,
        },
        FileReference {
            file: FileIdentity {
                path: path("meta/template.json"),
                sha256: Digest::new(
                    "sha256:3b23424744c85311b2b3b1aadb055bf20f72f623aee42696888a85f767fb1eda",
                )
                .expect("fixed manifest digest"),
                size: 8_850,
            },
            schema: Id::new("madb.project-template/1").expect("fixed template schema"),
        },
    )
}
