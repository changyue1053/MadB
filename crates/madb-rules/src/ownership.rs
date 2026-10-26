//! Exact ownership coverage over independently obtained requirements and plans.
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OwnerSetError {
    EmptyRequired,
    DuplicateKey,
    Mismatch,
}

/// Check coverage without discarding duplicate rows, including equal owners.
/// The caller establishes requirements independently of the provided plan.
pub fn compare_unique_owners<K: Ord, O: Eq>(
    required: &[(K, O)],
    provided: &[(K, O)],
) -> Result<(), OwnerSetError> {
    if required.is_empty() {
        return Err(OwnerSetError::EmptyRequired);
    }
    fn index<K: Ord, O>(rows: &[(K, O)]) -> Result<BTreeMap<&K, &O>, OwnerSetError> {
        let mut result = BTreeMap::new();
        for (key, owner) in rows {
            if result.insert(key, owner).is_some() {
                return Err(OwnerSetError::DuplicateKey);
            }
        }
        Ok(result)
    }
    if index(required)? != index(provided)? {
        return Err(OwnerSetError::Mismatch);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_unordered_coverage_and_owner() {
        let required = [("a", 1), ("b", 2)];
        assert_eq!(
            compare_unique_owners(&required, &[("b", 2), ("a", 1)]),
            Ok(())
        );
        for provided in [
            vec![("a", 1)],
            vec![("a", 1), ("b", 3)],
            vec![("a", 1), ("b", 2), ("c", 3)],
        ] {
            assert_eq!(
                compare_unique_owners(&required, &provided),
                Err(OwnerSetError::Mismatch)
            );
        }
    }

    #[test]
    fn duplicates_are_never_coalesced() {
        for rows in [[("a", 1), ("a", 1)], [("a", 1), ("a", 2)]] {
            assert_eq!(
                compare_unique_owners(&rows, &[("a", 1)]),
                Err(OwnerSetError::DuplicateKey)
            );
            assert_eq!(
                compare_unique_owners(&[("a", 1)], &rows),
                Err(OwnerSetError::DuplicateKey)
            );
        }
    }

    #[test]
    fn empty_requirements_precede_bad_provided_rows() {
        assert_eq!(
            compare_unique_owners(&[], &[("a", 1), ("a", 2)]),
            Err(OwnerSetError::EmptyRequired)
        );
    }
}
