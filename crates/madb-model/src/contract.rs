use crate::policy::{EffectivePolicy, Slot};
use crate::{Digest, Id, Location, PublicId, SourceId, SourceKey};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Direction {
    ConsumerToProvider,
    HostCallback,
    ReverseExport,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedSelector {
    pub route: Id,
    pub profile_id: Id,
    pub profile_digest: Digest,
    pub target: Id,
    pub direction: Direction,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Precision {
    CompilerDerived,
    ArtifactDerived,
    SchemaDerived,
    Heuristic,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CallKind {
    Sync,
    Async,
    Stream,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CallingConvention {
    C,
    System,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PublicIdOrigin {
    Explicit,
    Derived,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Parameter {
    pub name: Id,
    pub type_id: Id,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Symbol {
    pub source_key: SourceKey,
    pub source_id: SourceId,
    pub public_id: PublicId,
    pub public_id_origin: PublicIdOrigin,
    pub location: Location,
    pub precision: Precision,
    pub kind: CallKind,
    pub calling_convention: CallingConvention,
    pub link_name: Id,
    pub parameters: Vec<Parameter>,
    pub result: Id,
    pub throws: Option<Id>,
    pub constants: Vec<ExactConstant>,
    pub shape_digest: Digest,
}
impl Symbol {
    pub fn slot_type(&self, slot: &Slot) -> Option<&Id> {
        match slot {
            Slot::Call => None,
            Slot::Return => Some(&self.result),
            Slot::Parameter(name) => self
                .parameters
                .iter()
                .find(|p| &p.name == name)
                .map(|p| &p.type_id),
        }
    }
}

/// Backend-owned handles make no registry validity or pointer-dereference promise.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResourceRepresentation {
    RegistryU64,
    OpaquePointer,
    BackendHandle,
}

/// Type relationships are checked together by madb-rules; these are reader values.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TypeDefinition {
    Unit,
    Boolean,
    Integer {
        width: u16,
        signed: bool,
    },
    Float {
        width: u16,
    },
    String {
        encoding: String,
        max_bytes: u64,
    },
    Buffer {
        element: Id,
        length_unit: String,
        encoding: String,
        mutable: bool,
        max_bytes: u64,
    },
    Nullable {
        inner: Id,
    },
    Record {
        fields: Vec<Parameter>,
        c_layout: bool,
    },
    Variant {
        tag_type: Id,
        cases: Vec<VariantCase>,
        preserve_unknown: bool,
    },
    Resource {
        representation: ResourceRepresentation,
        owner: Id,
    },
    Callback {
        parameters: Vec<Parameter>,
        result: Id,
        throws: Option<Id>,
        calling_convention: CallingConvention,
    },
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VariantCase {
    pub name: Id,
    pub tag: ExactConstant,
    pub payload: Option<Id>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContractType {
    pub id: Id,
    pub definition: TypeDefinition,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExactConstant {
    Integer {
        signed: bool,
        width: u16,
        decimal: String,
    },
    Float {
        width: u16,
        bits: String,
    },
    String(String),
    Boolean(bool),
}

/// An immutable input value after wire conversion; semantic validity is a separate check.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Contract {
    pub namespace: Id,
    pub boundary_id: Id,
    pub selector: ResolvedSelector,
    pub source_digest: Digest,
    pub structure_digest: Digest,
    pub policy_digest: Digest,
    pub required_features: Vec<Id>,
    pub types: Vec<ContractType>,
    pub symbols: Vec<Symbol>,
    pub policies: Vec<EffectivePolicy>,
    pub observations: Vec<LayoutObservation>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Capability {
    pub selector: ResolvedSelector,
    pub supported_features: Vec<Id>,
    pub restrictions: Vec<(Id, String)>,
    pub producers: Vec<Producer>,
}

pub const CORE_FEATURES: &[&str] = &[
    "core.errors@1",
    "core.backend-resources@1",
    "core.values@1",
    "core.resources@1",
    "core.callbacks@1",
    "core.async@1",
    "core.streams@1",
    "core.explicit-close@1",
    "core.exact-numbers@1",
    "core.variants@1",
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Producer {
    pub id: Id,
    pub version: String,
    pub digest: Digest,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FieldLayout {
    pub name: Id,
    pub offset: u64,
    pub size: u64,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LayoutObservation {
    pub subject: Id,
    pub precision: Precision,
    pub input: Digest,
    pub selector: ResolvedSelector,
    pub producer: Producer,
    pub size: u64,
    pub alignment: u64,
    pub fields: Vec<FieldLayout>,
}
