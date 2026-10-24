use madb_model::contract::{PublicIdOrigin, Symbol};
use madb_model::{Finding, Id, PublicId};
use std::collections::BTreeSet;

pub fn validate_identities(
    namespace: &Id,
    symbols: &[Symbol],
    retired_ids: &[PublicId],
) -> Vec<Finding> {
    let mut findings = Vec::new();
    let mut public = BTreeSet::new();
    let mut source = BTreeSet::new();
    for symbol in symbols {
        let subject = symbol.public_id.as_str();
        if !public.insert(&symbol.public_id) {
            findings.push(
                Finding::new("ID001", subject, "duplicate public identity").at(&symbol.location),
            );
        }
        if !source.insert(&symbol.source_id) {
            findings.push(
                Finding::new("ID002", subject, "duplicate source identity").at(&symbol.location),
            );
        }
        if symbol.source_key.source_id().as_ref() != Ok(&symbol.source_id) {
            findings.push(
                Finding::new(
                    "ID003",
                    subject,
                    "source identity does not match stable reader key",
                )
                .at(&symbol.location),
            );
        }
        if symbol.public_id_origin == PublicIdOrigin::Derived
            && PublicId::derived(namespace, &symbol.source_id).as_ref() != Ok(&symbol.public_id)
        {
            findings.push(
                Finding::new(
                    "ID004",
                    subject,
                    "derived public ID changed without explicit stable-ID mapping",
                )
                .at(&symbol.location),
            );
        }
        if retired_ids.contains(&symbol.public_id) {
            findings.push(
                Finding::new("ID005", subject, "retired public identity cannot be reused")
                    .at(&symbol.location),
            );
        }
        if symbol.location.line == 0 || symbol.location.column == 0 {
            findings.push(
                Finding::new("ID006", subject, "source position is one-based").at(&symbol.location),
            );
        }
    }
    findings
}
