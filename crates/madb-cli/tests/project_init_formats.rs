//! I01-F composition tests. All roots and formal versions below are model
//! inputs; no Host, user directory, compiler, or initialization CLI is run.
use madb_app::inputs::capture_inputs;
use madb_app::ports::{FileReference, ReadWorkspace};
use madb_app::use_cases::initialize::{
    AbsolutePath, FrozenInitRequest, InitRequest, InitialFile, PreparedProject, VerifyLevel,
    freeze_request,
};
use madb_contracts::SchemaRegistry;
use madb_contracts::project_init::ProjectInitAdapter;
use madb_templates::{
    CheckedInventory, Digest, FileIdentity, FileOwnership, Id, LogicalPath, ParameterValue,
    parse_archive, render_asset,
};
use std::collections::BTreeMap;

const ARCHIVE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/project-init/template-design.tar"
));
const MANIFEST: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/project-init/package-input/meta/template.json"
));
const PACKAGE_SHA: &str = "sha256:2a47930bdb3f9298fc055469c8436f019fc648805dbdd3abcb94d78603d0a17b";
const MANIFEST_SHA: &str =
    "sha256:a5085f8a087bf6c1abe1bdf5183d7c384381402504f2f3811ebbcd6fa95fe034";

fn manifest_identity() -> FileIdentity {
    FileIdentity {
        path: LogicalPath::new("meta/template.json").unwrap(),
        sha256: Digest::new(MANIFEST_SHA).unwrap(),
        size: 8_848,
    }
}

fn checked_package(adapter: &ProjectInitAdapter) -> CheckedInventory {
    let template = adapter.decode_template(MANIFEST).unwrap();
    parse_archive(ARCHIVE.to_vec(), &Digest::new(PACKAGE_SHA).unwrap(), 43_008)
        .unwrap()
        .check_inventory(&manifest_identity(), &template.template().files)
        .unwrap()
}

fn request() -> InitRequest {
    InitRequest {
        template_selector: Some("org.madb.template/rust-c-sdk@0.1.0".into()),
        template_profile_selector: Some(Id::new("macos-arm64-values").unwrap()),
        parameter_values: BTreeMap::from([
            (
                Id::new("name").unwrap(),
                ParameterValue::String("hello_c".into()),
            ),
            (
                Id::new("namespace").unwrap(),
                ParameterValue::String("reading_demo".into()),
            ),
        ]),
        // The approved D rule selects the complete profile family set.
        targets: None,
        verify: VerifyLevel::Structure,
        open: None,
        dry_run: true,
        offline: true,
        output: AbsolutePath::new("/private/tmp/设计 项目/hello_c").unwrap(),
    }
}

fn freeze_model(adapter: &ProjectInitAdapter) -> FrozenInitRequest {
    let template = adapter.decode_template(MANIFEST).unwrap();
    // This is an explicitly modelled formal tool identity, never a mapping of
    // the actual development build to a supported formal version.
    freeze_request(&template, request(), "0.1.0", adapter)
        .unwrap()
        .0
}

fn initial_files(
    package: &CheckedInventory,
    frozen: &FrozenInitRequest,
) -> (Vec<InitialFile>, BTreeMap<LogicalPath, Vec<u8>>) {
    let selected = frozen.selected();
    let registry = SchemaRegistry::new().unwrap();
    let mut rendered = BTreeMap::new();
    for asset in selected
        .template()
        .assets
        .iter()
        .filter(|asset| selected.profile().assets.contains(&asset.id))
    {
        let bytes = render_asset(
            asset,
            package,
            &frozen.projection().parameter_values,
            &registry,
            |_| panic!("the selected design assets contain no MadB JSON output"),
        )
        .unwrap();
        if let Some(bytes) = bytes {
            assert!(rendered.insert(asset.output.clone(), bytes).is_none());
        }
    }
    let plan = selected.plan_with_provenance(&manifest_identity()).unwrap();
    let mut files = Vec::new();
    for planned in plan.files() {
        if planned.ownership == FileOwnership::LocalState {
            files.push(InitialFile::local_state(
                planned.module.clone(),
                planned.path.clone(),
            ));
            continue;
        }
        let bytes = if planned.path.as_str() == ".madb/template-package.tar" {
            package.archive().bytes().to_vec()
        } else if let Some(original) = planned.path.as_str().strip_prefix(".madb/template-input/") {
            package
                .archive()
                .entry_bytes(&LogicalPath::new(original).unwrap())
                .unwrap()
                .to_vec()
        } else {
            rendered.get(&planned.path).unwrap().clone()
        };
        files.push(
            InitialFile::rendered(
                planned.module.clone(),
                planned.path.clone(),
                planned.ownership,
                bytes,
            )
            .unwrap(),
        );
    }
    (files, rendered)
}

fn package_identity() -> FileIdentity {
    FileIdentity {
        path: LogicalPath::new("template-design.tar").unwrap(),
        sha256: Digest::new(PACKAGE_SHA).unwrap(),
        size: 43_008,
    }
}

struct PackageBytes(BTreeMap<LogicalPath, Vec<u8>>);
impl ReadWorkspace for PackageBytes {
    fn read(&self, path: &LogicalPath, limit: usize) -> madb_app::Result<Option<Vec<u8>>> {
        let bytes = self.0.get(path).cloned();
        assert!(bytes.as_ref().is_none_or(|value| value.len() <= limit));
        Ok(bytes)
    }
}

#[test]
fn nested_manifest_keeps_the_original_profile_and_sdk_root() {
    let adapter = ProjectInitAdapter::new().unwrap();
    let package = checked_package(&adapter);
    let request = freeze_model(&adapter);
    let references: Vec<_> = package
        .archive()
        .entries()
        .iter()
        .map(|entry| {
            let file = entry.identity();
            FileReference {
                path: file.path.clone(),
                digest: file.sha256.clone(),
                size: file.size,
            }
        })
        .collect();
    let source = PackageBytes(
        references
            .iter()
            .map(|file| {
                (
                    file.path.clone(),
                    package.archive().entry_bytes(&file.path).unwrap().to_vec(),
                )
            })
            .collect(),
    );
    let captured = capture_inputs(&source, references).unwrap();
    let template = request.selected().template();
    let bound = adapter
        .check_template_inputs(
            request.selected(),
            &manifest_identity(),
            &package_identity(),
            ARCHIVE,
            &captured,
            madb_contracts::project_init::CatalogTemplateIdentity {
                id: &template.id,
                version: template.version,
            },
        )
        .unwrap();
    assert_eq!(bound.profiles().len(), 1);
    assert_eq!(bound.profiles()[0].value()["sdk"]["path"], "toolchain.json");
    assert!(
        adapter
            .check_template_inputs(
                request.selected(),
                &manifest_identity(),
                &package_identity(),
                ARCHIVE,
                &captured,
                madb_contracts::project_init::CatalogTemplateIdentity {
                    id: &Id::new("unrelated-template").unwrap(),
                    version: template.version,
                },
            )
            .is_err()
    );
}

#[test]
fn original_package_selection_rendering_and_provenance_share_one_plan() {
    let adapter = ProjectInitAdapter::new().unwrap();
    let package = checked_package(&adapter);
    let frozen = freeze_model(&adapter);
    assert_eq!(frozen.projection().targets, [Id::new("macos").unwrap()]);
    let (files, rendered) = initial_files(&package, &frozen);
    assert_eq!(rendered.len(), 6);
    assert_eq!(
        rendered[&LogicalPath::new(".madb/project-parameters.json").unwrap()],
        b"{\"name\":\"hello_c\",\"namespace\":\"reading_demo\"}\n"
    );
    assert!(!rendered.contains_key(&LogicalPath::new(".madb/local-machine.json").unwrap()));
    let prepared =
        PreparedProject::new(&frozen, manifest_identity(), package_identity(), files).unwrap();
    assert_eq!(prepared.plan().files().len(), 17);
    assert_eq!(prepared.references().len(), 16);
    let saved_manifest = prepared
        .files()
        .iter()
        .find(|file| file.path().as_str() == ".madb/template-input/meta/template.json")
        .unwrap();
    assert_eq!(saved_manifest.bytes().unwrap(), MANIFEST);
}

#[test]
fn current_development_version_is_not_silently_promoted_to_formal() {
    let adapter = ProjectInitAdapter::new().unwrap();
    let template = adapter.decode_template(MANIFEST).unwrap();
    assert!(freeze_request(&template, request(), env!("CARGO_PKG_VERSION"), &adapter).is_err());
}

#[test]
fn changing_the_frozen_source_copy_cannot_form_a_prepared_project() {
    let adapter = ProjectInitAdapter::new().unwrap();
    let package = checked_package(&adapter);
    let frozen = freeze_model(&adapter);
    let (mut files, _) = initial_files(&package, &frozen);
    let original = files
        .iter_mut()
        .find(|file| file.path().as_str() == ".madb/template-input/profile.json")
        .unwrap();
    *original = InitialFile::rendered(
        original.module().clone(),
        original.path().clone(),
        original.ownership(),
        b"{}\n".to_vec(),
    )
    .unwrap();
    assert!(PreparedProject::new(&frozen, manifest_identity(), package_identity(), files).is_err());
}

#[test]
fn a_new_archive_pin_cannot_hide_an_unlisted_file() {
    let bytes = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/project-init/negative-package-unlisted-file.tar"
    ));
    let adapter = ProjectInitAdapter::new().unwrap();
    let template = adapter.decode_template(MANIFEST).unwrap();
    // Deliberately grant this different archive its own pin, isolating the
    // independent manifest inventory check from the original-byte hash check.
    let changed =
        parse_archive(bytes.to_vec(), &Digest::of_bytes(bytes), bytes.len() as u64).unwrap();
    assert!(
        changed
            .check_inventory(&manifest_identity(), &template.template().files)
            .is_err()
    );
}
