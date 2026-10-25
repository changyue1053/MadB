// Private scalar diagnostic frame. No handle or uint64 identity travels as number.
export const FRAME_FORMAT: number = 1;
export enum Frame {
  Format = 0, Phase = 1, BusinessEntries = 2, ScratchLive = 3,
  Operations = 4, Subscriptions = 5, EventTickets = 6, StateTickets = 7,
  Sources = 8, Entered = 9, Bytes = 10, Waiters = 11, Admissions = 12,
  Work = 13, Executors = 14, CoreClosing = 15, CoreClosed = 16,
  WorkerJoined = 17, JoinJobLive = 18, WaiterIndices = 19, ExplicitRefLive = 20,
  TsfnOwnerLive = 21, WakeLive = 22, InDispatch = 23, HookLive = 24,
  ContextLive = 25, ResidualErrors = 26, EnvExit = 27, ProviderContextLive = 28,
  CallbackContextsLive = 29, RuntimeCloseRequested = 30, MechanismTeardownStarted = 31,
  ReceiptConsumed = 32, ProviderAllocations = 33, ProviderReleases = 34,
  ProviderModelsLive = 35, ProviderWorkCalls = 36, WakeFailures = 37,
  WakeCreated = 38, WakeReleased = 39, ContextCreated = 40, ContextDestroyed = 41,
  RefCreated = 42, RefDeleted = 43, TsfnCreated = 44, TsfnFinalized = 45,
  HookAdded = 46, HookRetired = 47, JoinQueued = 48, JoinCompleted = 49,
  EnginesCreated = 50, EnginesDestroyed = 51, ScratchAcquired = 52, ScratchReleased = 53
}
// Phase 0=open, 1=runtime closing, 2=mechanism teardown requested,
// 3=physical native ACK (all required owners actually retired, no residual).
// Ordinary CoreClosed=1 is only business drain and is never this final ACK.
export function physicalRuntimeAck(frame: number[]): boolean {
  return frame.length === 54 && frame[Frame.Format] === FRAME_FORMAT && frame[Frame.Phase] === 3;
}
