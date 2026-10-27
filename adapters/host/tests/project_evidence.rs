#![cfg(target_os = "macos")]
use madb_app::{
    Digest, LogicalPath,
    ports::FileReference,
    use_cases::initialize::{AbsolutePath, FileIdentity, InitEvidencePort},
};
use madb_host::initialization::LocalInitEvidence;
fn root() -> AbsolutePath {
    AbsolutePath::new(
        madb_host::locations::temporary_parent()
            .unwrap()
            .join(madb_host::locations::unique_run_name().as_str())
            .to_str()
            .unwrap(),
    )
    .unwrap()
}
#[test]
fn separate_fixed_result_names_keep_complete_inventory_and_no_overwrite() {
    for project in [false, true] {
        let root = root();
        let mut store = LocalInitEvidence::create(&root).unwrap();
        let bytes = b"real log bytes";
        let file = FileIdentity {
            path: LogicalPath::new("run.log").unwrap(),
            sha256: Digest::of_bytes(bytes),
            size: bytes.len() as u64,
        };
        store
            .store_file(
                &FileReference {
                    path: file.path.clone(),
                    digest: file.sha256.clone(),
                    size: file.size,
                },
                bytes,
            )
            .unwrap();
        let result = if project {
            store.save_project_result(&root, b"actual project result", std::slice::from_ref(&file))
        } else {
            store.save_result(&root, b"actual init result", std::slice::from_ref(&file))
        }
        .unwrap();
        let name = if project {
            "project-result.json"
        } else {
            "init-result.json"
        };
        assert!(result.files.iter().any(|f| f.path.as_str() == name));
        assert_eq!(result.files.len(), 2);
        assert!(
            store
                .save_project_result(&root, b"replacement", std::slice::from_ref(&file))
                .is_err()
        );
        assert!(
            store
                .save_result(&root, b"replacement", std::slice::from_ref(&file))
                .is_err()
        );
        assert!(
            !std::fs::read(format!("{}/{name}", root.as_str()))
                .unwrap()
                .starts_with(b"replacement")
        );
        std::fs::remove_dir_all(root.as_str()).unwrap();
    }
}
#[test]
fn changed_saved_log_prevents_result_and_keeps_observed_bytes() {
    let root = root();
    let mut store = LocalInitEvidence::create(&root).unwrap();
    let file = FileIdentity {
        path: LogicalPath::new("run.log").unwrap(),
        sha256: Digest::of_bytes(b"original"),
        size: 8,
    };
    store
        .store_file(
            &FileReference {
                path: file.path.clone(),
                digest: file.sha256.clone(),
                size: file.size,
            },
            b"original",
        )
        .unwrap();
    std::fs::write(format!("{}/run.log", root.as_str()), b"modified").unwrap();
    assert!(
        store
            .save_project_result(&root, b"result", &[file])
            .is_err()
    );
    assert!(
        !std::path::Path::new(root.as_str())
            .join("project-result.json")
            .exists()
    );
    assert_eq!(
        std::fs::read(format!("{}/run.log", root.as_str())).unwrap(),
        b"modified"
    );
    std::fs::remove_dir_all(root.as_str()).unwrap();
}
