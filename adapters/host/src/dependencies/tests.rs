use super::*;
#[test]
fn depfiles_preserve_rules_and_escapes() {
    let parsed=parse_depfile(b"madb-dependencies: ../inputs/src/main.c \\\n ../inputs/inc/a\\ b.h ../inputs/inc/hash\\#.h\nmadb-dependencies: ../inputs/src/main.c\n").unwrap();
    assert_eq!(parsed.len(), 2);
    assert!(parsed[0].contains(&LogicalPath::new("inc/a b.h").unwrap()));
    assert_eq!(parsed[1].len(), 1);
}
#[test]
fn depfiles_reject_external_and_escape() {
    for bytes in [
        b"madb-dependencies: /etc/a.h\n".as_slice(),
        b"madb-dependencies: ../inputs/../../a.h\n",
        b"other: ../inputs/a.h\n",
    ] {
        assert!(parse_depfile(bytes).is_err());
    }
}
#[test]
fn unsupported_tokens_cannot_hide_behind_line_splicing() {
    for token in [
        "??",
        "%:",
        "##",
        "_Pragma",
        "__pragma",
        "#pragma",
        "#include_next",
        "#embed",
        "__has_embed",
    ] {
        let midpoint = token.len() / 2;
        for separator in ["\\\n", "\\\r\n", "\\ \n", "\\\t\n"] {
            let source = format!("{}{}{}", &token[..midpoint], separator, &token[midpoint..]);
            assert!(supported_source(source.as_bytes()).is_some(), "{source:?}");
        }
    }
    assert!(supported_source(b"#/**/include H").is_some());
    assert!(supported_source(b"#/**/include \"/outside.h\"").is_some());
}
#[test]
fn preparation_cancel_before_start_does_not_touch_filesystem() {
    let control = NativePreparationControl::default();
    control.cancel();
    let request = DependencyRequest {
        compiler: FileReference {
            path: LogicalPath::new("clang").unwrap(),
            digest: Digest::of_bytes(b"x"),
            size: 1,
        },
        target: Id::new("arm64-apple-macos11.0").unwrap(),
        sources: vec![LogicalPath::new("source/main.c").unwrap()],
        include_directories: vec![],
        absent: vec![],
        arguments: vec![],
        environment: BTreeMap::new(),
    };
    struct UnusedFormat;
    impl DependencyFormat for UnusedFormat {
        fn encode_request(&self, _: &DependencyRequest) -> Result<Vec<u8>> {
            panic!("cancelled before IO")
        }
        fn decode_request(&self, _: &[u8]) -> Result<DependencyRequest> {
            panic!("cancelled before IO")
        }
        fn encode_dependencies(&self, _: &DependencyManifest) -> Result<Vec<u8>> {
            panic!("cancelled before IO")
        }
        fn decode_dependencies(&self, _: &[u8]) -> Result<DependencyManifest> {
            panic!("cancelled before IO")
        }
        fn encode_cache(&self, _: &madb_app::dependencies::CacheManifest) -> Result<Vec<u8>> {
            panic!("cancelled before IO")
        }
        fn decode_cache(&self, _: &[u8]) -> Result<madb_app::dependencies::CacheManifest> {
            panic!("cancelled before IO")
        }
    }
    let missing = Path::new("/not-a-madb-workspace");
    let result = prepare_native_controlled(
        missing,
        missing,
        missing,
        &LogicalPath::new("run").unwrap(),
        &request,
        Digest::of_bytes(b"context"),
        &UnusedFormat,
        None,
        &control,
    );
    assert!(matches!(result,Err(error) if error.findings[0].code=="DEP006"));
}
#[test]
fn allocated_preparation_failure_reports_retained_directory() {
    let path = LogicalPath::new(format!("source/{}.h", "a".repeat(300))).unwrap();
    let file = FileReference {
        path: path.clone(),
        digest: Digest::of_bytes(b"x"),
        size: 1,
    };
    let snapshot = capture_inputs(&MemoryFile(path, b"x".to_vec()), vec![file]).unwrap();
    let root = std::env::temp_dir().join(format!("madb-dependency-partial-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    let root = std::fs::canonicalize(root).unwrap();
    let failure = match PreparedRun::create(&root, &LogicalPath::new("run").unwrap(), snapshot) {
        Ok(_) => panic!("overlong path must fail materialization"),
        Err(failure) => failure,
    };
    assert_eq!(
        failure.retained_path.as_deref(),
        Some(root.join("run").as_path())
    );
    let diagnostic = retained_preparation_error(failure);
    assert!(
        diagnostic
            .findings
            .iter()
            .any(|f| f.code == "DEP004" && f.message.contains(root.to_str().unwrap()))
    );
    assert!(root.join("run").is_dir());
    std::fs::remove_dir_all(&root).unwrap();
}
#[test]
fn computed_or_external_probes_have_no_proof() {
    for source in [
        "#include H",
        "#include \"../../x.h\"",
        "#if __has_include(\"/etc/passwd\")",
        "#define P __has_include",
        "#define P(a,b) a ## b",
        "# pragma clang module import X",
    ] {
        assert!(supported_source(source.as_bytes()).is_some(), "{source}");
    }
    assert!(supported_source(b"#include \"entry.h\"\n#if __has_include(\"optional.h\")\n#include \"optional.h\"\n#endif").is_none());
}
