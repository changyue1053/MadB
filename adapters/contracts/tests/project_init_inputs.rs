mod project_init_common;
use madb_app::inputs::capture_inputs;
use madb_app::ports::{FileReference, ReadWorkspace};
use madb_app::{LogicalPath, Result};
use madb_contracts::project_init::CatalogTemplateIdentity;
use madb_plan::project::Version;
use project_init_common::*;
use std::collections::BTreeMap;
struct Memory(BTreeMap<LogicalPath, Vec<u8>>);
impl ReadWorkspace for Memory {
    fn read(&self, path: &LogicalPath, _: usize) -> Result<Option<Vec<u8>>> {
        Ok(self.0.get(path).cloned())
    }
}
#[test]
fn original_manifest_and_sdk_paths_bind_from_one_package_root() {
    let a = adapter();
    let t = a
        .decode_template(&read("package-input/meta/template.json"))
        .unwrap();
    let s = t.select(None, None).unwrap();
    let manifest = file(
        "meta/template.json",
        &read("package-input/meta/template.json"),
    );
    let archive = read("template-design.tar");
    let package = file("template-design.tar", &archive);
    let mut inventory = t.template().files.clone();
    inventory.push(manifest.clone());
    let memory = Memory(
        inventory
            .iter()
            .map(|f| {
                (
                    f.path.clone(),
                    read(&format!("package-input/{}", f.path.as_str())),
                )
            })
            .collect(),
    );
    let refs = inventory
        .iter()
        .map(|f| FileReference {
            path: f.path.clone(),
            digest: f.sha256.clone(),
            size: f.size,
        })
        .collect();
    let frozen = capture_inputs(&memory, refs).unwrap();
    let binding = a
        .check_template_inputs(
            &s,
            &manifest,
            &package,
            &archive,
            &frozen,
            CatalogTemplateIdentity {
                id: &t.template().id,
                version: Version::parse("0.1.0").unwrap(),
            },
        )
        .unwrap();
    assert_eq!(binding.profiles().len(), 1);
    assert!(
        a.check_template_inputs(
            &s,
            &manifest,
            &package,
            &archive,
            &frozen,
            CatalogTemplateIdentity {
                id: &id("wrong-template"),
                version: Version::parse("0.1.0").unwrap()
            }
        )
        .is_err()
    );
    let mut changed = archive.clone();
    changed[512] ^= 1;
    assert!(
        a.check_template_inputs(
            &s,
            &manifest,
            &package,
            &changed,
            &frozen,
            CatalogTemplateIdentity {
                id: &t.template().id,
                version: Version::parse("0.1.0").unwrap()
            }
        )
        .is_err()
    );
}
#[test]
fn result_projection_matches_the_independent_adopted_digest() {
    use madb_app::use_cases::initialize::*;
    use madb_plan::project::ParameterValue;
    let a = adapter();
    let t = a
        .decode_template(&read("package-input/meta/template.json"))
        .unwrap();
    let mut r = request(true);
    r.output = AbsolutePath::new("/private/tmp/设计 项目/hello_c").unwrap();
    r.parameter_values
        .insert(id("name"), ParameterValue::String("hello_c".into()));
    r.parameter_values.insert(
        id("namespace"),
        ParameterValue::String("reading_demo".into()),
    );
    let (frozen, _) = freeze_request(&t, r, "0.1.0", &a).unwrap();
    let specimen: serde_json::Value =
        serde_json::from_slice(&read("vectors/positive-dry-run.json")).unwrap();
    assert_eq!(
        frozen.digest().as_str(),
        specimen["request_digest"].as_str().unwrap()
    );
}
