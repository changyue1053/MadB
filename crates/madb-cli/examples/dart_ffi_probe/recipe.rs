use super::{
    Result,
    inputs::{Inputs, id, path},
};
use madb_app::Digest;
use madb_contracts::{
    Document,
    composition::{
        AssetOrigin, BoundCompositionInputs, ComponentRequirement, DependencyRequirement,
        DependencyScope, Direction, GeneratedSlot, RecipeRequirements, RecipeSelection,
        ResolvedOwner, ResolvedResponsibility, ResolvedScope, ResolvedSubject, ResponsibilityKey,
        compare_responsibility_sets,
    },
};

pub const PACKAGE: &str = "org.madb.dart-ffi";
pub const BOUNDARY: &str = "dart-c-sync";
pub const PROFILE: &str = "dart-c-ffi-sync-aarch64-apple-darwin";
pub const READER: &str = "org.madb.dart-ffi/reader";
pub const GENERATOR: &str = "org.madb.dart-ffi/generator";
pub const SLOT: &str = "sync-source";
pub const RUNTIME: &str = "sdk/dart/ffi/madb_runtime.dart";
const NATIVE_PATHS: &[&str] = &[
    "provider/contract.json",
    "provider/identity.json",
    "provider/include/madb_rust.h",
    "provider/include/madb_rust_client.h",
    "provider/lib/libmadb_engine.dylib",
];
pub const SOURCE_PATHS: &[&str] = &[
    "LICENSE",
    "NOTICE",
    "README.md",
    "declarations.json",
    "lib/madb_dart_ffi.dart",
    "lib/src/generated.dart",
    "lib/src/madb_runtime.dart",
    "lib/src/raw_bindings.dart",
    "native/bridge.c",
    "native/bridge.h",
    "native/identity.json",
    "native/include/madb_rust.h",
    "native/include/madb_rust_client.h",
    "native/native.dylib",
    "pubspec.lock",
    "pubspec.yaml",
];
pub fn requirements() -> RecipeRequirements {
    RecipeRequirements {
        id: id("org.madb.dart-ffi/sync-source@1"),
        selection: RecipeSelection {
            profile_id: id(PROFILE),
            route: id("org.madb.dart.c-ffi"),
            direction: Direction::ConsumerToProvider,
            target: id("aarch64-apple-darwin"),
        },
        components: [(READER, "contract-reader"), (GENERATOR, "generator")]
            .into_iter()
            .map(|(name, port)| ComponentRequirement {
                id: id(name),
                port: id(port),
                interprets_composition: true,
            })
            .collect(),
        generated_slots: vec![GeneratedSlot {
            producer: id(GENERATOR),
            slot: id(SLOT),
            paths: SOURCE_PATHS.iter().map(|s| path(s)).collect(),
        }],
        dependencies: [
            ("facade", "runtime", DependencyScope::ConsumerCompile),
            ("facade", "runtime", DependencyScope::TargetRuntime),
            ("facade", "native", DependencyScope::TargetRuntime),
        ]
        .into_iter()
        .map(|(from, to, scope)| DependencyRequirement {
            from: id(from),
            to: id(to),
            scope,
        })
        .collect(),
    }
}
pub fn check(input: &Inputs, bound: &BoundCompositionInputs, ir: &Document) -> Result<()> {
    let required = roles(input, bound, ir)?;
    compare_responsibility_sets(&required, bound.responsibilities())?;
    Ok(())
}
pub fn check_assets(input: &Inputs, bound: &BoundCompositionInputs) -> Result<()> {
    let assets = bound.composition().value()["assets"]
        .as_array()
        .ok_or("assets")?;
    if assets.len() != 3 {
        return Err("fixed Dart recipe requires exactly three assets".into());
    }
    for (name, expected_paths) in [("runtime", &[RUNTIME][..]), ("native", NATIVE_PATHS)] {
        let asset = assets
            .iter()
            .find(|a| a["id"] == name)
            .ok_or("required asset absent")?;
        if asset["package"] != PACKAGE || asset["source"]["kind"] != "source" {
            return Err("asset origin differs from fixed recipe".into());
        }
        let refs = asset["source"]["files"]
            .as_array()
            .ok_or("source asset files")?;
        if refs.len() != expected_paths.len() {
            return Err("source asset closure differs".into());
        }
        for name in expected_paths {
            let reference = refs
                .iter()
                .find(|r| r["path"] == *name)
                .ok_or("fixed source path absent")?;
            let qualified = format!("packages/{PACKAGE}/0.1.0/{name}");
            let bytes = input
                .frozen
                .files()
                .find(|(r, _)| r.path.as_str() == qualified)
                .ok_or("fixed source bytes absent")?
                .1;
            if reference["sha256"] != Digest::of_bytes(bytes).as_str()
                || reference["size"] != bytes.len() as u64
            {
                return Err("fixed source reference differs from locked bytes".into());
            }
            if name.starts_with("provider/")
                && input.read(name, 64 * super::inputs::LIMIT)? != bytes
            {
                return Err("provider source and selected public candidate bytes differ".into());
            }
        }
    }
    let facade = assets
        .iter()
        .find(|a| a["id"] == "facade")
        .ok_or("facade asset absent")?;
    let mut paths = SOURCE_PATHS.to_vec();
    paths.sort();
    let expected = serde_json::json!({"kind":"generated","producer":GENERATOR,"output_slot":SLOT,"paths":paths});
    let mut actual = facade["source"].clone();
    actual["paths"]
        .as_array_mut()
        .ok_or("generated asset paths")?
        .sort_by_key(|v| v.as_str().unwrap_or("").to_owned());
    if facade["package"] != PACKAGE || actual != expected {
        return Err("generated asset closure differs from fixed slot".into());
    }
    Ok(())
}
pub fn roles(
    input: &Inputs,
    bound: &BoundCompositionInputs,
    ir: &Document,
) -> Result<Vec<ResolvedResponsibility>> {
    let model = madb_contracts::contract(ir)?;
    let names = [
        "bootstrap",
        "create",
        "checksum",
        "request_close",
        "await_closed",
        "release_handle",
    ];
    if model.symbols.len() != names.len() {
        return Err("Dart sync view requires exactly six roles".into());
    }
    let package = &bound.composition().value()["packages"][0];
    let digest = Digest::new(package["source_digest"].as_str().ok_or("source digest")?)?;
    let runtime_path = format!("packages/{PACKAGE}/0.1.0/{RUNTIME}");
    let runtime = input
        .frozen
        .files()
        .find(|(r, _)| r.path.as_str() == runtime_path)
        .ok_or("runtime source absent")?
        .1;
    if Digest::of_bytes(runtime)
        != Digest::of_bytes(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../sdk/dart/ffi/madb_runtime.dart"
        )))
    {
        return Err("runtime differs from the fixed host recipe's source".into());
    }
    let runtime_owner = ResolvedOwner::SdkAsset(AssetOrigin::Source {
        package: digest.clone(),
        paths: vec![path(RUNTIME)],
    });
    let mut paths = requirements().generated_slots.remove(0).paths;
    paths.sort();
    let facade = ResolvedOwner::SdkAsset(AssetOrigin::Generated {
        package: digest.clone(),
        producer: id(GENERATOR),
        slot: id(SLOT),
        paths,
    });
    let mut rows = Vec::new();
    for name in names {
        let public_id = format!("madb.engine.{name}.v1");
        let symbol = model
            .symbols
            .iter()
            .find(|s| s.public_id.as_str() == public_id)
            .ok_or("missing semantic role")?;
        let subject = ResolvedSubject::PublicSymbol(symbol.public_id.clone());
        for (stage, layer, owner) in [
            (
                "type-read",
                "header-reader",
                ResolvedOwner::Component {
                    id: id(READER),
                    package: digest.clone(),
                },
            ),
            (
                "abi-lowering",
                "ffi",
                ResolvedOwner::Component {
                    id: id(GENERATOR),
                    package: digest.clone(),
                },
            ),
            ("projection", "facade", facade.clone()),
            ("release", "temporary-call-memory", facade.clone()),
        ] {
            rows.push(ResolvedResponsibility {
                key: ResponsibilityKey {
                    boundary: id(BOUNDARY),
                    subject: subject.clone(),
                    stage: id(stage),
                    scope: ResolvedScope {
                        kind: id("call"),
                        anchor: subject.clone(),
                        layer: id(layer),
                    },
                },
                owner,
            });
        }
        if let Some(stage) = match name {
            "request_close" => Some("stop"),
            "await_closed" => Some("drain"),
            "release_handle" => Some("release"),
            _ => None,
        } {
            let create = model
                .symbols
                .iter()
                .find(|s| s.public_id.as_str() == "madb.engine.create.v1")
                .ok_or("create role")?;
            rows.push(ResolvedResponsibility {
                key: ResponsibilityKey {
                    boundary: id(BOUNDARY),
                    subject: subject.clone(),
                    stage: id(stage),
                    scope: ResolvedScope {
                        kind: id("resource"),
                        anchor: ResolvedSubject::PublicSymbol(create.public_id.clone()),
                        layer: id("dart-owner"),
                    },
                },
                owner: runtime_owner.clone(),
            });
        }
    }
    Ok(rows)
}
