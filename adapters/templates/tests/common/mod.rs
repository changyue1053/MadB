#![allow(dead_code)]
use madb_model::{Digest, LogicalPath};
use madb_plan::project::FileIdentity;
use madb_templates::{AssetError, CheckedArchive, parse_archive};

pub fn file(path: &str, bytes: &[u8]) -> FileIdentity {
    FileIdentity {
        path: LogicalPath::new(path).unwrap(),
        sha256: Digest::of_bytes(bytes),
        size: bytes.len() as u64,
    }
}

pub fn parse(bytes: Vec<u8>) -> Result<CheckedArchive, AssetError> {
    let digest = Digest::of_bytes(&bytes);
    let size = bytes.len() as u64;
    parse_archive(bytes, &digest, size)
}

pub fn archive(files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut bytes = Vec::new();
    for (path, data) in files {
        let mut header = [0; 512];
        if path.len() <= 100 {
            header[..path.len()].copy_from_slice(path.as_bytes());
        } else {
            let (prefix, name) = path.rsplit_once('/').unwrap();
            assert!(prefix.len() <= 155 && name.len() <= 100);
            header[..name.len()].copy_from_slice(name.as_bytes());
            header[345..345 + prefix.len()].copy_from_slice(prefix.as_bytes());
        }
        octal(&mut header[100..108], 0o644);
        octal(&mut header[108..116], 0);
        octal(&mut header[116..124], 0);
        octal(&mut header[124..136], data.len() as u64);
        octal(&mut header[136..148], 0);
        header[156] = b'0';
        header[257..265].copy_from_slice(b"ustar\x0000");
        checksum(&mut header);
        bytes.extend_from_slice(&header);
        bytes.extend_from_slice(data);
        bytes.resize(bytes.len().div_ceil(512) * 512, 0);
    }
    bytes.resize(bytes.len() + 1024, 0);
    bytes
}

pub fn octal(field: &mut [u8], number: u64) {
    let width = field.len() - 1;
    let text = format!("{number:0width$o}\0");
    assert_eq!(text.len(), field.len());
    field.copy_from_slice(text.as_bytes());
}

pub fn checksum(header: &mut [u8]) {
    header[148..156].fill(b' ');
    let total: u64 = header[..512].iter().map(|byte| u64::from(*byte)).sum();
    header[148..156].copy_from_slice(format!("{total:06o}\0 ").as_bytes());
}
