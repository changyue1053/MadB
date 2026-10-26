use madb_app::process::ProcessPolicy;
use madb_contracts::{
    DocumentKind, FormatError, SchemaRegistry,
    artifacts::ProvidedFile,
    extension::ExtensionManifest,
    extension_lock::{ComponentPinRequest, ExtensionPinContext, check_extension_pins},
    profile_digest, project_digest,
};
use madb_protocol::{Value, byte_digest};
use serde_json::json;

struct Fixture {
    project: Value,
    profile: Value,
    lock: Value,
    manifest: Value,
    image: Vec<u8>,
}
impl Fixture {
    fn new() -> Self {
        let registry = SchemaRegistry::new().unwrap();
        let manifest = json!({
            "schema":"madb.extension/1","id":"test.extension","version":"1.2.3",
            "compatibility":{"protocol":["1.0"],"ir":["1.0"]},"languages":[],
            "components":[{"id":"reader","port":"contract-reader","supported_features":["core.values@1"],
                "entrypoint":{"kind":"process","path":"bin/reader","args":["serve"]}}],
            "dependencies":[],"routes":[],"assets":[],
            "execution":{"transport":"stdio-jsonl","network":"none","env":["SDKROOT"]}
        });
        // These bytes are a data fixture, not a compiler or published package.
        let image = b"controlled component image\0v1".to_vec();
        let mut profile: Value = serde_json::from_str(include_str!(
            "../../../tests/fixtures/contracts/profile.json"
        ))
        .unwrap();
        profile["tools"] = json!([{"id":"test.extension/reader","version":"1.2.3", "sha256":byte_digest(&image),
            "host":"aarch64-apple-darwin","target":profile["target"],"source":"fixture://component/image","arguments":["serve"]}]);
        let mut project: Value = serde_json::from_str(include_str!(
            "../../../tests/fixtures/contracts/project.json"
        ))
        .unwrap();
        let mut lock: Value =
            serde_json::from_str(include_str!("../../../tests/fixtures/contracts/lock.json"))
                .unwrap();
        let profile_bytes = serde_json::to_vec(&profile).unwrap();
        let profile_file = file("profiles/rust-c.json", &profile_bytes);
        let mut profile_ref = profile_file;
        profile_ref["schema"] = json!("madb.profile/1");
        project["boundaries"][0]["profile"] = profile_ref.clone();
        lock["profiles"][0] = json!({"id":profile["id"],"semantic_digest":profile_digest(&registry.validate(DocumentKind::Profile, profile.clone()).unwrap()).unwrap(),"file":profile_ref});
        lock["project_digest"] = json!(
            project_digest(
                &registry
                    .validate(DocumentKind::Project, project.clone())
                    .unwrap()
            )
            .unwrap()
        );
        lock["tools"] = profile["tools"].clone();
        lock["components"] =
            json!([{"id":"test.extension/reader","version":"1.2.3","sha256":byte_digest(&image)}]);
        lock["inputs"].as_array_mut().unwrap().push(file(
            "extensions/test/manifest.json",
            &serde_json::to_vec(&manifest).unwrap(),
        ));
        Self {
            project,
            profile,
            lock,
            manifest,
            image,
        }
    }
    fn check(&self) -> Result<(), FormatError> {
        self.check_options(
            "aarch64-apple-darwin",
            "fixture://component/image",
            &["core.values@1"],
            true,
        )
    }
    fn check_options(
        &self,
        host: &str,
        source: &str,
        required: &[&str],
        grant: bool,
    ) -> Result<(), FormatError> {
        self.check_bytes(
            (
                &serde_json::to_vec(&self.manifest).unwrap(),
                &serde_json::to_vec(&self.profile).unwrap(),
                &self.image,
            ),
            host,
            source,
            required,
            grant,
        )
    }
    fn check_bytes(
        &self,
        (manifest_bytes, profile_bytes, image_bytes): (&[u8], &[u8], &[u8]),
        host: &str,
        source: &str,
        required: &[&str],
        grant: bool,
    ) -> Result<(), FormatError> {
        let registry = SchemaRegistry::new()?;
        let project = registry.validate(DocumentKind::Project, self.project.clone())?;
        let lock = registry.validate(DocumentKind::Lock, self.lock.clone())?;
        let registration = [ExtensionManifest::decode(manifest_bytes)?];
        let mut policy = ProcessPolicy::default();
        if grant {
            policy.allowed_environment.insert("SDKROOT".into());
        }
        let result = check_extension_pins(
            ExtensionPinContext {
                project: &project,
                lock: &lock,
                profile: ProvidedFile {
                    path: "profiles/rust-c.json",
                    bytes: profile_bytes,
                },
                boundary_id: "rust-to-c",
                policy: &policy,
            },
            ComponentPinRequest {
                registration: &registration,
                manifest: ProvidedFile {
                    path: "extensions/test/manifest.json",
                    bytes: manifest_bytes,
                },
                image: ProvidedFile {
                    path: "bin/reader",
                    bytes: image_bytes,
                },
                component_id: "reader",
                observed_host: host,
                observed_source: source,
                required_features: required,
            },
        )?;
        assert_eq!(result.component_id(), "reader");
        assert_eq!(result.manifest().id(), "test.extension");
        assert_eq!(result.image().digest.as_str(), byte_digest(image_bytes));
        assert_eq!(result.locked_tool()["source"], source);
        assert_eq!(
            result.profile_digest(),
            self.lock["profiles"][0]["semantic_digest"]
                .as_str()
                .unwrap()
        );
        Ok(())
    }
    fn repin_profile(&mut self) {
        let registry = SchemaRegistry::new().unwrap();
        let bytes = serde_json::to_vec(&self.profile).unwrap();
        let mut profile_ref = file("profiles/rust-c.json", &bytes);
        profile_ref["schema"] = json!("madb.profile/1");
        self.project["boundaries"][0]["profile"] = profile_ref.clone();
        self.lock["profiles"][0]["file"] = profile_ref;
        self.lock["profiles"][0]["semantic_digest"] = json!(
            profile_digest(
                &registry
                    .validate(DocumentKind::Profile, self.profile.clone())
                    .unwrap()
            )
            .unwrap()
        );
        self.lock["project_digest"] = json!(
            project_digest(
                &registry
                    .validate(DocumentKind::Project, self.project.clone())
                    .unwrap()
            )
            .unwrap()
        );
    }
    fn repin_manifest(&mut self) {
        self.lock["inputs"][1] = file(
            "extensions/test/manifest.json",
            &serde_json::to_vec(&self.manifest).unwrap(),
        );
    }
}
fn file(path: &str, bytes: &[u8]) -> Value {
    json!({"path":path,"sha256":byte_digest(bytes),"size":bytes.len()})
}
fn reject(result: Result<(), FormatError>, code: &str) {
    let error = result.expect_err("expected rejection");
    assert_eq!(error.code, code, "{error}");
}

#[test]
fn exact_registered_component_image_and_profile_pins_are_checked() {
    Fixture::new().check().unwrap();
}

#[test]
fn sources_hosts_features_and_environment_are_not_inferred_or_granted() {
    let fixture = Fixture::new();
    reject(
        fixture.check_options(
            "other-host",
            "fixture://component/image",
            &["core.values@1"],
            true,
        ),
        "LOCK004",
    );
    reject(
        fixture.check_options(
            "aarch64-apple-darwin",
            "fixture://other/image",
            &["core.values@1"],
            true,
        ),
        "LOCK004",
    );
    reject(
        fixture.check_options(
            "aarch64-apple-darwin",
            "fixture://component/image",
            &["unknown.required@1"],
            true,
        ),
        "CapabilityUnsupported",
    );
    reject(
        fixture.check_options(
            "aarch64-apple-darwin",
            "fixture://component/image",
            &["core.values@1"],
            false,
        ),
        "InputInvalid",
    );
    let mut network = Fixture::new();
    network.manifest["execution"]["network"] = json!("requested");
    network.repin_manifest();
    reject(network.check(), "CapabilityUnsupported");
}

#[test]
fn profile_tool_tuple_changes_cannot_reuse_the_old_lock() {
    for (key, value) in [
        ("version", json!("2.0.0")),
        ("target", json!("other-target")),
        ("source", json!("fixture://changed")),
        ("arguments", json!(["other"])),
    ] {
        let mut fixture = Fixture::new();
        fixture.profile["tools"][0][key] = value;
        fixture.repin_profile();
        reject(fixture.check(), "LOCK004");
    }
}

#[test]
fn exact_component_mapping_and_manifest_entrypoint_are_required() {
    let mut fixture = Fixture::new();
    fixture.lock["components"][0]["version"] = json!("2.0.0");
    reject(fixture.check(), "ExtensionIdentityMismatch");
    let mut fixture = Fixture::new();
    fixture.lock["components"][0]["id"] = json!("test.other/reader");
    reject(fixture.check(), "LOCK004");
    let mut fixture = Fixture::new();
    fixture.manifest["components"][0]["entrypoint"]["path"] = json!("other/image");
    fixture.repin_manifest();
    reject(fixture.check(), "ExtensionIdentityMismatch");
    let mut fixture = Fixture::new();
    fixture.manifest["components"][0]["entrypoint"]["args"] = json!(["changed"]);
    fixture.repin_manifest();
    reject(fixture.check(), "ExtensionIdentityMismatch");
}

#[test]
fn missing_manifest_pin_and_duplicate_or_case_alias_pins_fail() {
    let mut fixture = Fixture::new();
    fixture.lock["inputs"].as_array_mut().unwrap().pop();
    reject(fixture.check(), "LOCK005");
    for key in ["profiles", "tools", "components", "inputs"] {
        let mut fixture = Fixture::new();
        let first = fixture.lock[key][0].clone();
        fixture.lock[key].as_array_mut().unwrap().push(first);
        reject(fixture.check(), "LOCK006");
    }
    let mut fixture = Fixture::new();
    let mut pin = fixture.lock["inputs"][1].clone();
    pin["path"] = json!("extensions/test/Manifest.json");
    fixture.lock["inputs"].as_array_mut().unwrap().push(pin);
    reject(fixture.check(), "LOCK006");
}

#[test]
fn changed_bytes_and_project_or_profile_identity_are_rejected() {
    let mut fixture = Fixture::new();
    fixture.image.push(0);
    reject(fixture.check(), "LOCK004");
    let mut fixture = Fixture::new();
    fixture.project["sdk_namespace"] = json!("changed");
    reject(fixture.check(), "LOCK002");
    let mut fixture = Fixture::new();
    fixture.profile["deployment_target"] = json!("changed");
    reject(fixture.check(), "FILE004");
    let mut fixture = Fixture::new();
    fixture.manifest["version"] = json!("1.2.4");
    reject(fixture.check(), "FILE004");
    let mut fixture = Fixture::new();
    fixture.lock["profiles"][0]["semantic_digest"] = json!(byte_digest(b"different"));
    reject(fixture.check(), "LOCK003");
}

#[test]
fn actual_file_bytes_are_rehashed_without_json_reserialization() {
    let fixture = Fixture::new();
    let root = std::env::temp_dir().join(format!("madb-extension-pins-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    let manifest = serde_json::to_vec(&fixture.manifest).unwrap();
    let profile = serde_json::to_vec(&fixture.profile).unwrap();
    for (name, bytes) in [
        ("manifest.json", manifest.as_slice()),
        ("profile.json", profile.as_slice()),
        ("image", fixture.image.as_slice()),
    ] {
        std::fs::write(root.join(name), bytes).unwrap();
    }
    fixture
        .check_bytes(
            (
                &std::fs::read(root.join("manifest.json")).unwrap(),
                &std::fs::read(root.join("profile.json")).unwrap(),
                &std::fs::read(root.join("image")).unwrap(),
            ),
            "aarch64-apple-darwin",
            "fixture://component/image",
            &["core.values@1"],
            true,
        )
        .unwrap();
    std::fs::write(
        root.join("manifest.json"),
        serde_json::to_vec_pretty(&fixture.manifest).unwrap(),
    )
    .unwrap();
    reject(
        fixture.check_bytes(
            (
                &std::fs::read(root.join("manifest.json")).unwrap(),
                &profile,
                &fixture.image,
            ),
            "aarch64-apple-darwin",
            "fixture://component/image",
            &["core.values@1"],
            true,
        ),
        "FILE004",
    );
    std::fs::remove_dir_all(root).unwrap();
}
