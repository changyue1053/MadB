//! Approved projection vectors are format fixtures, not executable candidates.
use madb_protocol::{DigestDomain, Value, canonical_json, semantic_digest, strict_json};
use std::path::PathBuf;

#[test]
fn project_build_domains_match_independent_known_bytes() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/project-build/digest-vectors.json");
    let vectors: Value = strict_json(&std::fs::read(path).unwrap()).unwrap();
    for vector in vectors["vectors"].as_array().unwrap() {
        let domain = match vector["domain"].as_str().unwrap() {
            "madb.project-build-request/1" => DigestDomain::ProjectBuildRequest,
            "madb.project-build-input/1" => DigestDomain::ProjectBuildInput,
            "madb.project-build-output/1" => DigestDomain::ProjectBuildOutput,
            other => panic!("unexpected projection domain: {other}"),
        };
        let bytes = canonical_json(&vector["projection"]).unwrap();
        assert_eq!(
            bytes,
            vector["canonical_projection"].as_str().unwrap().as_bytes(),
            "{}",
            vector["id"]
        );
        let mut preimage = domain.as_str().as_bytes().to_vec();
        preimage.push(b'\n');
        preimage.extend_from_slice(&bytes);
        assert_eq!(
            preimage.len() as u64,
            vector["preimage_size"].as_u64().unwrap()
        );
        let hex: String = preimage.iter().map(|byte| format!("{byte:02x}")).collect();
        assert_eq!(hex, vector["preimage_hex"].as_str().unwrap());
        assert_eq!(
            semantic_digest(domain, &vector["projection"]).unwrap(),
            vector["digest"].as_str().unwrap(),
            "{}",
            vector["id"]
        );
        // A projection cannot be reused under a different identity domain.
        assert_ne!(
            semantic_digest(DigestDomain::InitRequest, &vector["projection"]).unwrap(),
            vector["digest"].as_str().unwrap()
        );
    }
}
