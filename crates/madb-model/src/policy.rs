use crate::{Id, Location, PublicId, SourceId};
use std::collections::BTreeMap;

macro_rules! vocabulary {
    ($name:ident { $($variant:ident => $text:literal),+ $(,)? }) => {
        #[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
        pub enum $name { $($variant),+ }
        impl $name {
            pub fn as_str(self) -> &'static str { match self { $(Self::$variant => $text),+ } }
            pub fn parse(value: &str) -> Option<Self> { match value { $($text => Some(Self::$variant)),+, _ => None } }
        }
    }
}
vocabulary!(Ownership { Copy => "copy", Borrow => "borrow", Transfer => "transfer", Shared => "shared" });
vocabulary!(BorrowScope { Call => "call", Operation => "operation", Subscription => "subscription" });
vocabulary!(KeepAlive { CallFrame => "call-frame", OperationOwner => "operation-owner", SourceLease => "source-lease" });
vocabulary!(Threading { Caller => "caller", SerialExecutor => "serial-executor", DedicatedThread => "dedicated-thread", UiExecutor => "ui-executor", RuntimeEnvironment => "runtime-environment" });
vocabulary!(Blocking { NonBlocking => "non-blocking", Bounded => "bounded", Unbounded => "unbounded" });
vocabulary!(Delivery { Registered => "registered", Accepted => "accepted", WorkCompleted => "work-completed", ResultDelivered => "result-delivered" });
vocabulary!(Cancellation { None => "none", Wait => "wait", Observe => "observe", Work => "work" });
vocabulary!(CancellationConfirmation { NotSupported => "not-supported", WaitEnded => "wait-ended", Stopped => "stopped", OperationTerminal => "operation-terminal" });
vocabulary!(Close { None => "none", RequestAwaitRelease => "request-await-release" });
vocabulary!(ContextOwner { Caller => "caller", SourceLease => "source-lease", SubscriptionOwner => "subscription-owner" });
vocabulary!(Stop { None => "none", RequestAndDrain => "request-and-drain", DeliveryGateWithSourceLease => "delivery-gate-with-source-lease" });
vocabulary!(StreamKind { None => "none", State => "state", Event => "event" });
vocabulary!(Overflow { Reject => "reject", CoalesceLatest => "coalesce-latest" });
vocabulary!(Ordering { Source => "source", Executor => "executor" });
vocabulary!(Replay { None => "none", Latest => "latest" });
vocabulary!(Recovery { None => "none", Resubscribe => "resubscribe", DomainProtocol => "domain-protocol" });
vocabulary!(ReleaseThread { Caller => "caller", DedicatedThread => "dedicated-thread", RuntimeEnvironment => "runtime-environment", OwnerExecutor => "owner-executor" });
vocabulary!(Authority { SourceAnnotation => "source-annotation", ProjectPolicy => "project-policy", PolicyFamily => "policy-family", ProfileDefault => "profile-default" });

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum Slot {
    Call,
    Return,
    Parameter(Id),
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExactSelector {
    Public(PublicId),
    Source(SourceId),
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Origin {
    pub authority: Authority,
    pub id: Id,
    pub location: Location,
}

// Each value carries its own key, so a string cannot silently change a boolean
// or ownership field. No backend-specific Any/Map enters the domain model.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PolicyValue {
    Ownership(Ownership),
    Releaser(PublicId),
    BorrowScope(BorrowScope),
    KeepAlive(KeepAlive),
    Threading(Threading),
    Blocking(Blocking),
    Delivery(Delivery),
    Cancellation(Cancellation),
    CancellationConfirmation(CancellationConfirmation),
    Close(Close),
    SynchronousCallback(bool),
    Reentrant(bool),
    ContextOwner(ContextOwner),
    Stop(Stop),
    StreamKind(StreamKind),
    CapacityItems(u64),
    CapacityBytes(u64),
    Overflow(Overflow),
    Ordering(Ordering),
    Replay(Replay),
    Recovery(Recovery),
    ReleaseThread(ReleaseThread),
}
impl PolicyValue {
    pub fn key(&self) -> &'static str {
        match self {
            Self::Ownership(_) => "ownership",
            Self::Releaser(_) => "releaser",
            Self::BorrowScope(_) => "borrow_scope",
            Self::KeepAlive(_) => "keep_alive",
            Self::Threading(_) => "threading",
            Self::Blocking(_) => "blocking",
            Self::Delivery(_) => "delivery",
            Self::Cancellation(_) => "cancellation",
            Self::CancellationConfirmation(_) => "cancellation_confirmation",
            Self::Close(_) => "close",
            Self::SynchronousCallback(_) => "synchronous_callback",
            Self::Reentrant(_) => "reentrant",
            Self::ContextOwner(_) => "context_owner",
            Self::Stop(_) => "stop",
            Self::StreamKind(_) => "stream_kind",
            Self::CapacityItems(_) => "capacity_items",
            Self::CapacityBytes(_) => "capacity_bytes",
            Self::Overflow(_) => "overflow",
            Self::Ordering(_) => "ordering",
            Self::Replay(_) => "replay",
            Self::Recovery(_) => "recovery",
            Self::ReleaseThread(_) => "release_thread",
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Binding {
    pub selector: ExactSelector,
    pub slot: Slot,
    pub families: Vec<Id>,
    pub values: Vec<PolicyValue>,
    pub origin: Origin,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Family {
    pub id: Id,
    pub values: Vec<PolicyValue>,
    pub origin: Origin,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourcedValue {
    pub value: PolicyValue,
    pub origin: Origin,
    pub replaced_default: Option<Origin>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EffectivePolicy {
    pub symbol_id: PublicId,
    pub slot: Slot,
    pub values: BTreeMap<&'static str, SourcedValue>,
}
