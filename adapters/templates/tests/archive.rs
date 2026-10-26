mod common;
use common::{archive, checksum, file, octal, parse};
use madb_model::{Digest, LogicalPath};
use madb_plan::project::FileIdentity;
use madb_protocol::{DocumentKind, SchemaRegistry};
use madb_templates::{AssetErrorKind, parse_archive};

const DESIGN: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/project-init/template-design.tar"
));
const MANIFEST: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/project-init/package-input/meta/template.json"
));

#[test]
fn original_design_bytes_and_complete_inventory_are_verified() {
    let expected =
        Digest::new("sha256:2a47930bdb3f9298fc055469c8436f019fc648805dbdd3abcb94d78603d0a17b")
            .unwrap();
    let parsed = parse_archive(DESIGN.to_vec(), &expected, 43008).unwrap();
    assert_eq!(parsed.bytes(), DESIGN);
    assert_eq!(parsed.entries().len(), 9);
    let document = SchemaRegistry::new()
        .unwrap()
        .decode(DocumentKind::ProjectTemplate, MANIFEST)
        .unwrap();
    let files: Vec<_> = document.value()["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| FileIdentity {
            path: LogicalPath::new(value["path"].as_str().unwrap()).unwrap(),
            sha256: Digest::new(value["sha256"].as_str().unwrap()).unwrap(),
            size: value["size"].as_u64().unwrap(),
        })
        .collect();
    let package = parsed
        .check_inventory(&file("meta/template.json", MANIFEST), &files)
        .unwrap();
    assert_eq!(package.manifest_bytes(), MANIFEST);
    let profile = files
        .iter()
        .find(|file| file.path.as_str() == "profile.json")
        .unwrap();
    let profile_bytes = package.referenced_bytes(profile).unwrap();
    let decoded = SchemaRegistry::new()
        .unwrap()
        .decode(DocumentKind::Profile, profile_bytes)
        .unwrap();
    let sdk = &decoded.value()["sdk"];
    let sdk = FileIdentity {
        path: LogicalPath::new(sdk["path"].as_str().unwrap()).unwrap(),
        sha256: Digest::new(sdk["sha256"].as_str().unwrap()).unwrap(),
        size: sdk["size"].as_u64().unwrap(),
    };
    assert_eq!(sdk.path.as_str(), "toolchain.json");
    assert!(!package.referenced_bytes(&sdk).unwrap().is_empty());
}

#[test]
fn archive_identity_is_over_original_bytes_not_repacked_content() {
    let bytes = archive(&[("manifest.json", b"{}")]);
    let size = bytes.len() as u64;
    assert_eq!(
        parse_archive(bytes.clone(), &Digest::of_bytes(b"{}"), size)
            .unwrap_err()
            .kind,
        AssetErrorKind::ArchiveIdentity
    );
    assert_eq!(
        parse_archive(bytes.clone(), &Digest::of_bytes(&bytes), size + 1)
            .unwrap_err()
            .kind,
        AssetErrorKind::ArchiveIdentity
    );
    let mut changed = bytes.clone();
    changed[512] = b'[';
    assert_eq!(
        parse_archive(changed, &Digest::of_bytes(&bytes), size)
            .unwrap_err()
            .kind,
        AssetErrorKind::ArchiveIdentity
    );
}

#[test]
fn inventory_is_exact_including_unselected_entries_and_nested_root() {
    let entries: &[(&str, &[u8])] = &[
        ("meta/template.json", b"{}"),
        ("sdk.json", b"{}"),
        ("unselected.txt", b"unselected"),
    ];
    let bytes = archive(entries);
    let manifest = file("meta/template.json", b"{}");
    let files = [
        file("sdk.json", b"{}"),
        file("unselected.txt", b"unselected"),
    ];
    let package = parse(bytes.clone())
        .unwrap()
        .check_inventory(&manifest, &files)
        .unwrap();
    assert!(
        package
            .referenced_bytes(&file("meta/sdk.json", b"{}"))
            .is_err()
    );
    assert!(package.referenced_bytes(&manifest).is_err());
    assert!(
        parse(bytes.clone())
            .unwrap()
            .check_inventory(&manifest, &files[..1])
            .is_err()
    );
    let mut wrong = files.clone();
    wrong[1].sha256 = Digest::of_bytes(b"other");
    assert!(
        parse(bytes.clone())
            .unwrap()
            .check_inventory(&manifest, &wrong)
            .is_err()
    );
    wrong = files.clone();
    wrong[0].size += 1;
    assert!(
        parse(bytes.clone())
            .unwrap()
            .check_inventory(&manifest, &wrong)
            .is_err()
    );
    wrong = files.clone();
    wrong.reverse();
    assert!(
        parse(bytes.clone())
            .unwrap()
            .check_inventory(&manifest, &wrong)
            .is_err()
    );
    assert!(
        parse(bytes)
            .unwrap()
            .check_inventory(&manifest, &[manifest.clone(), files[1].clone()])
            .is_err()
    );
}

#[test]
fn restricted_header_rejects_other_types_owners_encodings_and_unused_data() {
    for (offset, replacement) in [
        (156, b'1'),
        (156, b'2'),
        (156, b'5'),
        (156, b'x'),
        (156, b'L'),
        (156, 0),
        (257, b'g'),
        (263, b'1'),
        (157, b'a'),
        (265, b'a'),
        (297, b'a'),
        (500, 1),
        (124, 0x80),
        (100, b' '),
        (107, b' '),
        (108, b'1'),
        (116, b'1'),
        (136, b'1'),
        (101, b'1'),
        (329, b'1'),
        (337, b'1'),
        (0, 0xff),
        (8, b'x'),
    ] {
        let mut bytes = archive(&[("a", b"data")]);
        bytes[offset] = replacement;
        checksum(&mut bytes[..512]);
        assert!(
            parse(bytes).is_err(),
            "header offset {offset}, replacement {replacement}"
        );
    }
    let mut bytes = archive(&[("a", b"data")]);
    octal(&mut bytes[329..337], 0);
    octal(&mut bytes[337..345], 0);
    checksum(&mut bytes[..512]);
    assert!(parse(bytes).is_ok(), "canonical device zeros are permitted");
}

#[test]
fn checksum_padding_truncation_and_exact_trailer_are_required() {
    let original = archive(&[("a", b"data")]);
    let mut wrong_checksum = original.clone();
    wrong_checksum[148] = b'7';
    let mut wrong_padding = original.clone();
    wrong_padding[516] = 1;
    let mut one_trailer = original.clone();
    one_trailer.truncate(one_trailer.len() - 512);
    let mut extra_trailer = original.clone();
    extra_trailer.extend([0; 512]);
    let mut trailing = original.clone();
    trailing.extend([1; 512]);
    let mut short = original.clone();
    short.pop();
    let mut hidden = original.clone();
    hidden[1536] = 1;
    let mut truncated = original.clone();
    octal(&mut truncated[124..136], 8192);
    checksum(&mut truncated[..512]);
    for invalid in [
        wrong_checksum,
        wrong_padding,
        one_trailer,
        extra_trailer,
        trailing,
        short,
        hidden,
        truncated,
        vec![0; 1024],
    ] {
        assert!(parse(invalid).is_err());
    }
}

#[test]
fn paths_reject_traversal_aliases_conflicts_and_ordering() {
    for path in [
        "/a", "a//b", "a/../b", "a/./b", "a\\b", "C:a", "a\nb", "a/", "",
    ] {
        assert!(parse(archive(&[(path, b"data")])).is_err(), "{path:?}");
    }
    for entries in [
        vec![("A/x", b"a".as_slice()), ("a/y", b"b".as_slice())],
        vec![("a", b"a".as_slice()), ("a/b", b"b".as_slice())],
        vec![("a", b"a".as_slice()), ("a", b"b".as_slice())],
        vec![("b", b"a".as_slice()), ("a", b"b".as_slice())],
    ] {
        assert!(parse(archive(&entries)).is_err());
    }
    let path = vec!["a"; 33].join("/");
    assert!(parse(archive(&[(&path, b"data")])).is_err());
    let path = format!("{}/{}", "a".repeat(120), "b".repeat(50));
    assert!(
        parse(archive(&[(&path, b"data")])).is_ok(),
        "valid USTAR prefix joins at package root"
    );
}

#[test]
fn budgets_reject_before_unbounded_materialization() {
    let mut oversized_file = archive(&[("a", b"")]);
    octal(&mut oversized_file[124..136], 32 * 1024 * 1024 + 1);
    checksum(&mut oversized_file[..512]);
    assert!(parse(oversized_file).is_err());
    let paths: Vec<_> = (0..4097).map(|n| format!("f{n:04}")).collect();
    let entries: Vec<_> = paths
        .iter()
        .map(|path| (path.as_str(), b"".as_slice()))
        .collect();
    assert!(parse(archive(&entries)).is_err());
    assert!(parse(archive(&entries[..4096])).is_ok());
    let paths: Vec<_> = (0..1025)
        .map(|n| format!("d{n:04}/a/b/c/d/e/f/g/file"))
        .collect();
    let entries: Vec<_> = paths
        .iter()
        .map(|path| (path.as_str(), b"".as_slice()))
        .collect();
    assert!(
        parse(archive(&entries)).is_err(),
        "8200 distinct parent directories exceed 8192"
    );
    let oversized_archive = vec![0; 133 * 1024 * 1024 + 512];
    assert!(parse_archive(oversized_archive, &Digest::of_bytes(b"unused"), 0).is_err());
}

#[test]
fn total_content_budget_counts_all_real_payloads() {
    let large = vec![b'x'; 32 * 1024 * 1024];
    let bytes = archive(&[
        ("a", &large),
        ("b", &large),
        ("c", &large),
        ("d", &large),
        ("manifest", b"x"),
    ]);
    assert!(bytes.len() < 133 * 1024 * 1024);
    let error = parse(bytes).unwrap_err();
    assert_eq!(error.kind, AssetErrorKind::ArchiveFormat);
    assert!(error.message.contains("total file content"));
}
