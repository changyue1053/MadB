use madb_model::{Digest, Id, PublicId, SourceId, ValueError};
use madb_protocol::{Document, DocumentKind, FormatError, Value};

#[derive(Clone)]
pub(crate) struct Wire<'a> {
    pub value: &'a Value,
    pub path: String,
}
impl<'a> Wire<'a> {
    pub fn root(document: &'a Document, kind: DocumentKind) -> Result<Self, FormatError> {
        if document.kind() != kind {
            return Err(FormatError::new(
                "FMT003",
                "/schema",
                "wrong document kind for conversion",
            ));
        }
        Ok(Self {
            value: document.value(),
            path: String::new(),
        })
    }
    pub fn at(value: &'a Value, path: impl Into<String>) -> Self {
        Self {
            value,
            path: path.into(),
        }
    }
    pub fn field(&self, name: &str) -> Result<Self, FormatError> {
        let path = format!(
            "{}/{}",
            self.path,
            name.replace('~', "~0").replace('/', "~1")
        );
        self.value
            .get(name)
            .map(|value| Self {
                value,
                path: path.clone(),
            })
            .ok_or_else(|| FormatError::new("DTO001", path, "required wire field absent"))
    }
    pub fn string(&self) -> Result<&'a str, FormatError> {
        self.value
            .as_str()
            .ok_or_else(|| self.error("expected string"))
    }
    pub fn uint(&self) -> Result<u64, FormatError> {
        self.value
            .as_u64()
            .ok_or_else(|| self.error("expected unsigned integer"))
    }
    pub fn int(&self) -> Result<i64, FormatError> {
        self.value
            .as_i64()
            .ok_or_else(|| self.error("expected integer"))
    }
    pub fn boolean(&self) -> Result<bool, FormatError> {
        self.value
            .as_bool()
            .ok_or_else(|| self.error("expected boolean"))
    }
    pub fn array(&self) -> Result<Vec<Self>, FormatError> {
        self.value
            .as_array()
            .map(|items| {
                items
                    .iter()
                    .enumerate()
                    .map(|(i, value)| Self {
                        value,
                        path: format!("{}/{i}", self.path),
                    })
                    .collect()
            })
            .ok_or_else(|| self.error("expected array"))
    }
    pub fn object(&self) -> Result<Vec<(&'a str, Self)>, FormatError> {
        self.value
            .as_object()
            .map(|items| {
                items
                    .iter()
                    .map(|(key, value)| {
                        (
                            key.as_str(),
                            Self {
                                value,
                                path: format!("{}/{}", self.path, key),
                            },
                        )
                    })
                    .collect()
            })
            .ok_or_else(|| self.error("expected object"))
    }
    pub fn id(&self) -> Result<Id, FormatError> {
        Id::new(self.string()?).map_err(|e| self.domain_error(e))
    }
    pub fn public_id(&self) -> Result<PublicId, FormatError> {
        PublicId::new(self.string()?).map_err(|e| self.domain_error(e))
    }
    pub fn source_id(&self) -> Result<SourceId, FormatError> {
        SourceId::new(self.string()?).map_err(|e| self.domain_error(e))
    }
    pub fn digest(&self) -> Result<Digest, FormatError> {
        Digest::new(self.string()?).map_err(|e| self.domain_error(e))
    }
    pub fn choice<T>(&self, parse: impl FnOnce(&str) -> Option<T>) -> Result<T, FormatError> {
        parse(self.string()?).ok_or_else(|| self.error("unknown semantic value"))
    }
    pub fn error(&self, message: impl Into<String>) -> FormatError {
        FormatError::new("DTO001", &self.path, message)
    }
    pub fn domain_error(&self, e: ValueError) -> FormatError {
        FormatError::new("DTO002", &self.path, e.to_string())
    }
}
