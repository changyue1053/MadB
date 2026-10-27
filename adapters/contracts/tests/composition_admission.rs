// Reuse the established explicit-package fixture and its unchanged regressions.
include!("composition.rs");
use madb_app::process::ProcessPolicy;
use madb_contracts::composition::admission::{CompositionImageRequest, check_composition_image};
use madb_contracts::execution::{ExecutionRecipe, ExecutionTool};

fn image_fixture() -> (Fixture, ExecutionRecipe) {
    let mut f = Fixture::new();
    let image = b"separately installed executable";
    f.profile["tools"] = json!([{"id":"test.image","version":"1.0.0","sha256":byte_digest(image),
        "host":"aarch64-apple-darwin","target":"aarch64-linux-ohos","source":"explicit-local-root","arguments":[]}]);
    f.repin_profile();
    let recipe = ExecutionRecipe {
        id: id("test.generate"),
        request: FileReference {
            path: path("request.json"),
            digest: Digest::of_bytes(b"{}"),
            size: 2,
        },
        tool: ExecutionTool {
            id: id("test.image"),
            version: "1.0.0".into(),
            digest: Digest::of_bytes(image),
        },
        installed_tool: FileReference {
            path: path("installed-peer"),
            digest: Digest::of_bytes(image),
            size: image.len() as u64,
        },
        arguments: vec![],
        environment: BTreeMap::new(),
        working_directory: path("work"),
        response_path: path("outputs.json"),
        inputs: vec![],
        input_schemas: BTreeMap::new(),
    };
    (f, recipe)
}
fn image_check(
    f: &Fixture,
    r: &ExecutionRecipe,
    host: &str,
    source: &str,
    image: &[u8],
    features: &[&str],
) -> Result<(), FormatError> {
    let bound = f.check()?;
    check_composition_image(
        &bound,
        CompositionImageRequest {
            entrypoint_source: "bin/generator",
            component_id: "test.composed/generator",
            recipe: r,
            installed_image: ProvidedFile {
                path: "installed-peer",
                bytes: image,
            },
            observed_host: host,
            observed_source: source,
            required_features: features,
            policy: &ProcessPolicy::default(),
        },
    )
    .map(|_| ())
}
#[test]
fn composition_keeps_package_source_and_installed_image_distinct() {
    let (f, r) = image_fixture();
    assert_ne!(
        f.composition["components"][0]["sha256"],
        r.tool.digest.as_str()
    );
    image_check(
        &f,
        &r,
        "aarch64-apple-darwin",
        "explicit-local-root",
        b"separately installed executable",
        &["core.extension-composition@1"],
    )
    .unwrap();
    let manifest =
        madb_contracts::extension::ExtensionManifest::decode(&encode(&f.manifest)).unwrap();
    assert_eq!(
        madb_contracts::extension::require_component_features(
            &manifest,
            "generator",
            &["core.extension-composition@1"]
        )
        .unwrap_err()
        .code,
        "CapabilityUnsupported"
    );
}
#[test]
fn composition_exact_image_host_source_and_recipe_rejections() {
    let (f, mut r) = image_fixture();
    let image = b"separately installed executable";
    let features = ["core.extension-composition@1"];
    for (host, source, bytes) in [
        (
            "x86_64-unknown-linux-gnu",
            "explicit-local-root",
            image.as_slice(),
        ),
        ("aarch64-apple-darwin", "another-root", image.as_slice()),
        (
            "aarch64-apple-darwin",
            "explicit-local-root",
            b"wrong executable".as_slice(),
        ),
    ] {
        assert_eq!(
            image_check(&f, &r, host, source, bytes, &features)
                .unwrap_err()
                .code,
            "LOCK004"
        );
    }
    r.arguments.push("unexpected".into());
    assert_eq!(
        image_check(
            &f,
            &r,
            "aarch64-apple-darwin",
            "explicit-local-root",
            image,
            &features
        )
        .unwrap_err()
        .code,
        "LOCK004"
    );
    r.arguments.clear();
    r.tool.digest = Digest::new(
        f.composition["packages"][0]["source_digest"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        image_check(
            &f,
            &r,
            "aarch64-apple-darwin",
            "explicit-local-root",
            image,
            &features
        )
        .unwrap_err()
        .code,
        "LOCK004"
    );
}
#[test]
fn composition_mandatory_features_and_permissions_reject() {
    let (mut f, r) = image_fixture();
    let image = b"separately installed executable";
    for features in [
        vec![],
        vec!["unknown.feature@1"],
        vec![
            "core.extension-composition@1",
            "core.extension-composition@1",
        ],
    ] {
        assert_eq!(
            image_check(
                &f,
                &r,
                "aarch64-apple-darwin",
                "explicit-local-root",
                image,
                &features
            )
            .unwrap_err()
            .code,
            "CapabilityUnsupported"
        );
    }
    f.manifest["execution"]["env"] = json!(["UNGRANTED_ENV"]);
    f.repin_package();
    assert_eq!(
        image_check(
            &f,
            &r,
            "aarch64-apple-darwin",
            "explicit-local-root",
            image,
            &["core.extension-composition@1"]
        )
        .unwrap_err()
        .code,
        "InputInvalid"
    );
}
