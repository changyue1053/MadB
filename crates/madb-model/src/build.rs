//! Host build completion is independent of downstream runtime certification.
use crate::Id;

pub const EXTERNAL_RUNTIME_VERIFICATION: &str = "core.external-runtime-verification@1";

/// Task phases retain their existing identity and order when shared by planning
/// and the build-completion policy. Wire conversion belongs to adapters.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Phase {
    Metadata,
    Generate,
    Compile,
    Link,
    Run,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BuildCompletionPolicy {
    CompileLinkRun,
    ExternalRuntimeVerification,
}
impl BuildCompletionPolicy {
    /// Callers validate feature support before resolving this selection.
    /// Merely advertising support must never change the selected policy.
    pub fn from_required_features(required: &[Id]) -> Self {
        if required
            .iter()
            .any(|id| id.as_str() == EXTERNAL_RUNTIME_VERIFICATION)
        {
            Self::ExternalRuntimeVerification
        } else {
            Self::CompileLinkRun
        }
    }

    pub fn required_phases(self) -> &'static [Phase] {
        match self {
            Self::CompileLinkRun => &[Phase::Compile, Phase::Link, Phase::Run],
            Self::ExternalRuntimeVerification => &[Phase::Compile, Phase::Link],
        }
    }

    pub fn allows(self, phase: Phase) -> bool {
        match self {
            // Existing graphs may also contain metadata/generate tasks.
            Self::CompileLinkRun => true,
            Self::ExternalRuntimeVerification => matches!(phase, Phase::Compile | Phase::Link),
        }
    }

    pub fn is_complete(self, phases: impl IntoIterator<Item = Phase>) -> bool {
        let phases: Vec<_> = phases.into_iter().collect();
        phases.iter().all(|phase| self.allows(*phase))
            && self
                .required_phases()
                .iter()
                .all(|phase| phases.contains(phase))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_requires_compile_link_run_and_preserves_extra_phases() {
        let policy = BuildCompletionPolicy::from_required_features(&[]);
        assert_eq!(policy, BuildCompletionPolicy::CompileLinkRun);
        assert!(!policy.is_complete([Phase::Compile, Phase::Link]));
        assert!(policy.is_complete([
            Phase::Metadata,
            Phase::Generate,
            Phase::Compile,
            Phase::Link,
            Phase::Run
        ]));
    }

    #[test]
    fn explicit_external_runtime_requires_two_phases_and_forbids_other_work() {
        let policy = BuildCompletionPolicy::from_required_features(&[Id::new(
            EXTERNAL_RUNTIME_VERIFICATION,
        )
        .unwrap()]);
        assert_eq!(policy, BuildCompletionPolicy::ExternalRuntimeVerification);
        assert!(policy.is_complete([Phase::Link, Phase::Compile, Phase::Compile]));
        assert!(!policy.is_complete([Phase::Compile]));
        for forbidden in [Phase::Metadata, Phase::Generate, Phase::Run] {
            assert!(!policy.allows(forbidden));
            assert!(!policy.is_complete([Phase::Compile, Phase::Link, forbidden]));
        }
    }
}
