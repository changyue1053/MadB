use super::*;
use crate::{Digest, LogicalPath, Result};
use std::collections::{BTreeMap, BTreeSet};

/// Captured source bytes only. Host separately retains its filesystem stamps;
/// decoding a public input inventory cannot construct this value.
#[derive(Clone, Debug)]
pub struct CapturedInputs {
    files: BTreeMap<LogicalPath, Vec<u8>>,
    directories: Vec<InputDirectory>,
}
impl CapturedInputs {
    pub fn new(
        files: BTreeMap<LogicalPath, Vec<u8>>,
        mut directories: Vec<InputDirectory>,
    ) -> Result<Self> {
        if files.len() > 4096
            || files.values().any(|b| b.len() > 32 * 1024 * 1024)
            || files.values().map(|b| b.len()).sum::<usize>() > 128 * 1024 * 1024
        {
            return Err(invalid("inputs", "finite project source budget exceeded"));
        }
        for name in [
            DESCRIPTOR,
            ORIGIN,
            ".madb/template-package.tar",
            ".madb/template-input/profile.json",
            ".madb/template-input/toolchain.json",
            "rust/src/lib.rs",
            "consumer/main.c",
        ] {
            if !files.keys().any(|p| p.as_str() == name) {
                return Err(invalid(name, "required project input is missing"));
            }
        }
        directories.sort_by_key(|a| utf16(a.path.as_str()));
        if directories
            .iter()
            .map(|d| d.path.as_str())
            .collect::<Vec<_>>()
            != ["consumer/include", "rust/src"]
        {
            return Err(invalid(
                "directories",
                "both finite source directories must be observed exactly once",
            ));
        }
        let mut total_members = 0;
        for directory in &mut directories {
            directory.members.sort_by_key(|a| utf16(a.path.as_str()));
            let mut seen = BTreeSet::new();
            for member in &directory.members {
                total_members += 1;
                if !directory.present
                    || !seen.insert(&member.path)
                    || member.path.as_str().split('/').count() > 64
                {
                    return Err(invalid(
                        directory.path.as_str(),
                        "invalid directory membership",
                    ));
                }
                let source = format!("{}/{}", directory.path.as_str(), member.path.as_str());
                if member.kind == MemberKind::File && !files.keys().any(|p| p.as_str() == source) {
                    return Err(invalid(source, "directory member has no captured bytes"));
                }
            }
            if directory.path.as_str() == "rust/src"
                && (!directory.present
                    || directory.members.len() != 1
                    || directory.members[0].path.as_str() != "lib.rs"
                    || directory.members[0].kind != MemberKind::File)
            {
                return Err(invalid(
                    "rust/src",
                    "this recipe supports only the single natural lib.rs source",
                ));
            }
        }
        if total_members > 16384 {
            return Err(invalid("directories", "directory member budget exceeded"));
        }
        Ok(Self { files, directories })
    }
    pub fn files(&self) -> &BTreeMap<LogicalPath, Vec<u8>> {
        &self.files
    }
    pub fn directories(&self) -> &[InputDirectory] {
        &self.directories
    }
    pub fn inventory(&self) -> Result<InputInventory> {
        let mut files = self
            .files
            .iter()
            .map(|(source, bytes)| {
                Ok(InputFile {
                    source: source.clone(),
                    file: FileIdentity {
                        path: LogicalPath::new(format!("inputs/{}", source.as_str()))
                            .map_err(|e| invalid(source.as_str(), e.to_string()))?,
                        sha256: Digest::of_bytes(bytes),
                        size: bytes.len() as u64,
                    },
                })
            })
            .collect::<Result<Vec<_>>>()?;
        files.sort_by_key(|a| utf16(a.source.as_str()));
        Ok(InputInventory {
            files,
            directories: self.directories.clone(),
        })
    }
}
fn utf16(value: &str) -> Vec<u16> {
    value.encode_utf16().collect()
}
