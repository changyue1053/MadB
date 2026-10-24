use sha2::{Digest as _, Sha256};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValueError(pub &'static str);
impl std::fmt::Display for ValueError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}
impl std::error::Error for ValueError {}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Id(String);
impl Id {
    pub fn new(value: impl Into<String>) -> Result<Self, ValueError> {
        let value = value.into();
        if value.is_empty()
            || value.len() > 512
            || !value.as_bytes()[0].is_ascii_alphanumeric()
            || !value
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"._:/@+-".contains(&c))
        {
            return Err(ValueError("invalid opaque identifier"));
        }
        Ok(Self(value))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl std::fmt::Display for Id {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct PublicId(Id);
impl PublicId {
    pub fn new(value: impl Into<String>) -> Result<Self, ValueError> {
        Id::new(value).map(Self)
    }
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
    pub fn derived(namespace: &Id, source: &SourceId) -> Result<Self, ValueError> {
        Self::new(format!("{}.{}", namespace.as_str(), &source.as_str()[10..]))
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct SourceId(String);
impl SourceId {
    pub fn new(value: impl Into<String>) -> Result<Self, ValueError> {
        let value = value.into();
        if !value.starts_with("source:v1:") || !lower_hex(&value[10..], 64) {
            return Err(ValueError("invalid source identity"));
        }
        Ok(Self(value))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Reader-defined stable declaration key. Locations and shapes are intentionally absent.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceKey {
    pub reader: Id,
    pub package: String,
    pub module: String,
    pub kind: String,
    pub qualified_name: String,
    pub overload: String,
}
impl SourceKey {
    pub fn source_id(&self) -> Result<SourceId, ValueError> {
        let mut hash = Sha256::new();
        hash.update(b"madb.source-id/1\n");
        for component in [
            self.reader.as_str(),
            &self.package,
            &self.module,
            &self.kind,
            &self.qualified_name,
            &self.overload,
        ] {
            if component.is_empty() || component.len() > 4096 || component.contains('\0') {
                return Err(ValueError("incomplete stable source key"));
            }
            hash.update((component.len() as u64).to_be_bytes());
            hash.update(component.as_bytes());
        }
        Ok(SourceId(format!("source:v1:{}", hex(&hash.finalize()))))
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct Digest(String);
impl Digest {
    pub fn new(value: impl Into<String>) -> Result<Self, ValueError> {
        let value = value.into();
        if !value.starts_with("sha256:") || !lower_hex(&value[7..], 64) {
            return Err(ValueError("invalid SHA-256 identity"));
        }
        Ok(Self(value))
    }
    pub fn of_bytes(bytes: &[u8]) -> Self {
        Self(format!("sha256:{}", hex(&Sha256::digest(bytes))))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
fn lower_hex(value: &str, len: usize) -> bool {
    value.len() == len
        && value
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct LogicalPath(String);
impl LogicalPath {
    pub fn new(value: impl Into<String>) -> Result<Self, ValueError> {
        let value = value.into();
        if value.is_empty()
            || value.len() > 1024
            || value.contains(['\\', ':'])
            || value.chars().any(char::is_control)
            || value
                .split('/')
                .any(|part| part.is_empty() || part == "." || part == "..")
        {
            return Err(ValueError("expected a normalized logical relative path"));
        }
        Ok(Self(value))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Location {
    pub path: LogicalPath,
    pub line: u64,
    pub column: u64,
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
