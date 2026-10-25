"""Reviewed test-only facade composition; ordinary source contains no controls."""


def replace_once(source, before, after):
    if source.count(before) != 1:
        raise ValueError("Kotlin companion source seam changed: " + before)
    return source.replace(before, after)


def derive_boundary(boundary):
    s = boundary
    s = replace_once(
        s,
        "  private requireOpen(): void {",
        "  testKey(): string { return this.runtime; }\n  private requireOpen(): void {",
    )
    s = replace_once(
        s,
        "  connect(): Engine { return this.session.connect(); }",
        "  testKey(): string { return this.session.testKey(); }\n  connect(): Engine { return this.session.connect(); }",
    )
    s = replace_once(
        s,
        "  roundtrip(value: Snapshot): Snapshot { return this.session.roundtrip(this.record, value); }",
        "  testKey(): string { return this.session.testKey(); }\n  roundtrip(value: Snapshot): Snapshot { return this.session.roundtrip(this.record, value); }",
    )
    s = controls_boundary(s)
    s += """
// Test profile only. These are public HAR exports with checked wrapper brands.
function checkedRuntime(value: Runtime): RuntimeHandle {
  if (!(value instanceof RuntimeHandle)) { throw new BoundaryError('InvalidArgument'); }
  return value as RuntimeHandle;
}
function checkedEngine(value: Engine): EngineHandle {
  if (!(value instanceof EngineHandle)) { throw new BoundaryError('InvalidArgument'); }
  return value as EngineHandle;
}
export function testObserve(runtime: Runtime): number[] { return native.auditKey(checkedRuntime(runtime).testKey()); }
export function testExportKey(runtime: Runtime): string { return checkedRuntime(runtime).testKey(); }
export function testInspect(key: string): number[] { return native.auditKey(key); }
export interface SavedCalls {
  peek(runtime: string, id: string): number;
  takeStatus(runtime: string, id: string): number;
  connect(runtime: string): string;
  abort(runtime: string): number[];
}
export function testSaveCalls(runtime: Runtime): SavedCalls { return native.saveCalls(checkedRuntime(runtime).testKey()); }
export function testObserveNapi(): string { return native.napiAudit(); }
export function testRawFrame(): number[] { return native.audit(); }
export function testLastAdmissionAudit(): number[] { return native.bootstrapAudit(); }
export function testPollAdmission(): number[] { return native.pollAdmission(); }
export function testArmPartialOpenFailure(stage: number): void { native.armPartial(stage); }
export function testArmWork(engine: Engine, phase: number): void {
  if (phase !== 1 && phase !== 2) { throw new BoundaryError('CapabilityUnsupported'); }
  native.testGate(checkedEngine(engine).testKey(), phase === 1 ? 0 : 2, 1);
}
export function testReleaseWork(engine: Engine): void { native.testGate(checkedEngine(engine).testKey(), 0, 0); native.testGate(checkedEngine(engine).testKey(), 2, 0); }
export function testHoldPublication(engine: Engine): void { native.testGate(checkedEngine(engine).testKey(), 1, 1); }
export function testReleasePublication(engine: Engine): void { native.testGate(checkedEngine(engine).testKey(), 1, 0); }
"""
    return s + CONTROL_EXPORTS


def derive_port(port):
    s = replace_once(
        port,
        "  audit(): number[] {",
        "  saveCalls(runtime: string): SavedCalls { bootstrap.audit(runtime); const api: OwnedApi = this.current(); return new SavedCallsOwner(api); }\n  testGate(runtime: string, kind: number, enabled: number): void { this.current().ownedTestGate(runtime, kind, enabled); }\n  auditKey(runtime: string): number[] { return bootstrap.audit(runtime); }\n  armPartial(stage: number): void { bootstrap.testArmPartialOpen(stage); }\n  pollAdmission(): number[] { return bootstrap.testPollAdmission(); }\n  napiAudit(): string { return bootstrap.testNapiAudit(); }\n  memoryAudit(): string { return bootstrap.testMemoryAudit(); }\n  audit(): number[] {",
    )
    s = replace_once(
        s,
        "class KotlinPort {",
        "export interface SavedCalls { peek(runtime: string, id: string): number; takeStatus(runtime: string, id: string): number; connect(runtime: string): string; abort(runtime: string): number[]; }\nclass SavedCallsOwner implements SavedCalls { private readonly api: OwnedApi; constructor(api: OwnedApi) { this.api = api; } peek(runtime: string, id: string): number { return this.api.ownedPeek(runtime, id); } takeStatus(runtime: string, id: string): number { return this.api.ownedTakeStatus(runtime, id); } connect(runtime: string): string { return this.api.ownedConnect(runtime); } abort(runtime: string): number[] { return bootstrap.abortAdmission(runtime); } }\nclass KotlinPort {",
    )
    return replace_once(s, "  audit(): number[] {", CONTROL_PORT + "  audit(): number[] {")


def derive_declaration(declaration):
    s = replace_once(
        declaration,
        "  audit(): number[];",
        "  audit(runtime?: string): number[];\n  testArmPartialOpen(stage: number): number;\n  testPollAdmission(): number[];\n  testNapiAudit(): string;\n  testMemoryAudit(): string;",
    )
    return s


CONTROL_EXPORTS = """
function checkedOperation(value: Operation): OperationHandle {
  if (!(value instanceof OperationHandle)) { throw new BoundaryError('InvalidArgument'); }
  return value as OperationHandle;
}
function checkedSubscription(value: Subscription): SubscriptionHandle {
  if (!(value instanceof SubscriptionHandle)) { throw new BoundaryError('InvalidArgument'); }
  return value as SubscriptionHandle;
}
export function testMemoryFrame(): string { return native.memoryAudit(); }
export function testControlFrame(runtime: Runtime): string { return native.testInspect(checkedRuntime(runtime).testKey()); }
export function testArmDelivery(subscription: Subscription): void { native.testDelivery(checkedSubscription(subscription).testResource(), 0); }
export function testDeliveryFrame(subscription: Subscription): string { return native.testDelivery(checkedSubscription(subscription).testResource(), 1); }
export function testReleaseDelivery(subscription: Subscription): void { native.testDelivery(checkedSubscription(subscription).testResource(), 2); }
export function testStagePublication(engine: Engine, stream: number, snapshot: Snapshot): void {
  native.testPublication(snapshot, checkedEngine(engine).testResource(), stream);
}
export function testFinishPublication(engine: Engine, commit: boolean): void { native.testReleasePublication(checkedEngine(engine).testResource(), commit ? 1 : 0); }
export function testArmCloneFailure(engine: Engine, nth: number, ownedError: boolean): void { native.testClone(checkedEngine(engine).testResource(), nth, ownedError ? 1 : 0); }
export function testArmTransferFailure(runtime: Runtime, phase: number): void { native.testTransfer(checkedRuntime(runtime).testKey(), phase); }
export function testEngineHandle(engine: Engine, probeCase: number): void { native.testHandle(checkedEngine(engine).testResource(), 0, probeCase); }
export function testOperationHandle(operation: Operation, probeCase: number): void { native.testHandle(checkedOperation(operation).testResource(), 1, probeCase); }
export function testSubscriptionState(subscription: Subscription): number[] { return checkedSubscription(subscription).testState(); }
export function testSubscriptionHandle(subscription: Subscription, probeCase: number): void { native.testHandle(checkedSubscription(subscription).testResource(), 2, probeCase); }
export function testSettleProbe(runtime: Runtime, action: () => void, throwAfter: boolean): void { checkedRuntime(runtime).testSettleProbe(action, throwAfter); }
export function testSettleInspect(runtime: Runtime): number[] { return checkedRuntime(runtime).testSettleInspect(); }
"""

CONTROL_PORT = """  testInspect(runtime: string): string { return this.current().ownedTestInspect(runtime); }
  testDelivery(subscription: string, action: number): string { return this.current().ownedTestDelivery(subscription, action); }
  testPublication(snapshot: Snapshot, engine: string, stream: number): void { this.current().ownedTestPublication(snapshot, engine, stream); }
  testReleasePublication(engine: string, commit: number): void { this.current().ownedTestReleasePublication(engine, commit); }
  testClone(engine: string, nth: number, mode: number): void { this.current().ownedTestClone(engine, nth, mode); }
  testTransfer(runtime: string, phase: number): void { this.current().ownedTestTransfer(runtime, phase); }
  testHandle(handle: string, kind: number, probeCase: number): void { this.current().ownedTestHandle(handle, kind, probeCase); }
"""


def controls_boundary(source):
    s = source
    for member in (
        "  roundtrip(value: Snapshot): Snapshot { return this.session.roundtrip(this.record, value); }",
        "  result(): Promise<Snapshot> { return this.session.result(this.record); }",
        "  requestStop(): void { this.session.requestClose(this.record); }",
    ):
        s = replace_once(
            s, member, "  testResource(): string { return this.record.handle; }\n" + member
        )
    s = replace_once(
        s,
        "  requestStop(): void { this.session.requestClose(this.record); }",
        "  testState(): number[] { return [1, this.record.closing ? 1 : 0, this.record.closed ? 1 : 0, this.record.listener === undefined ? 0 : 1]; }\n  requestStop(): void { this.session.requestClose(this.record); }",
    )
    s = replace_once(
        s,
        "  testKey(): string { return this.session.testKey(); }\n  connect(): Engine",
        """  testKey(): string { return this.session.testKey(); }
  testSettleProbe(action: (() => void) | undefined, throwAfter: boolean): void { this.session.testSettleProbe(action, throwAfter); }
  testSettleInspect(): number[] { return this.session.testSettleInspect(); }
  connect(): Engine""",
    )
    s = replace_once(
        s,
        "  private closePromise: Promise<void> | undefined = undefined;",
        """  private settleAction: (() => void) | undefined = undefined;
  private settleThrow: boolean = false;
  private settleArmed: boolean = false;
  private settleArms: number = 0;
  private settleHits: number = 0;
  private settleResolve: number = 0;
  private settleReject: number = 0;
  private settleExceptions: number = 0;
  private settleMaxObserved: number = 0;
  private closePromise: Promise<void> | undefined = undefined;""",
    )
    s = replace_once(
        s,
        "  private requireOpen(): void {",
        """  testSettleProbe(action: (() => void) | undefined, throwAfter: boolean): void {
    this.requireOpen();
    if (action !== undefined && typeof action !== 'function') { throw new BoundaryError('InvalidArgument'); }
    if (this.settleArmed) { throw new BoundaryError('ResourceExhausted'); }
    this.settleAction = action; this.settleThrow = throwAfter; this.settleArmed = true; this.settleArms++;
  }
  testSettleInspect(): number[] {
    let current: number = 0;
    this.waiters.forEach((waiter: Waiter) => { if (waiter.settling) { current++; } });
    const values: number[] = [this.settleArms, this.settleHits, this.settleResolve, this.settleReject,
      this.settleExceptions, current, this.settleMaxObserved];
    for (let i: number = 0; i < values.length; i++) {
      if (!Number.isSafeInteger(values[i]) || values[i] < 0) { throw new BoundaryError('MetricRangeExceeded'); }
    }
    return values;
  }
  private afterSettler(accepted: boolean): void {
    if (accepted) { this.settleResolve++; } else { this.settleReject++; }
    if (!this.settleArmed) { return; }
    const action: (() => void) | undefined = this.settleAction;
    const throwAfter: boolean = this.settleThrow;
    this.clearSettler(); this.settleHits++;
    this.settleMaxObserved = Math.max(this.settleMaxObserved, this.waiters.size);
    try {
      if (action !== undefined) { action(); }
      if (throwAfter) { throw new BoundaryError('BridgeFailure'); }
    } catch (error) { this.settleExceptions++; throw error as Error; }
  }
  private clearSettler(): void { this.settleAction = undefined; this.settleThrow = false; this.settleArmed = false; }
  private requireOpen(): void {""",
    )
    s = replace_once(
        s,
        "    waiter.accept(copy);",
        "    waiter.accept(copy);\n    this.afterSettler(copy.status === 0);",
    )
    s = replace_once(
        s,
        "        waiter.accept({ status: status, value: 0, text: '' });",
        "        waiter.accept({ status: status, value: 0, text: '' });\n        this.afterSettler(status === 0);",
    )
    s = replace_once(
        s,
        "        waiter.reject(nativeError(reason));",
        "        waiter.reject(nativeError(reason));\n        this.afterSettler(false);",
    )
    s = replace_once(
        s,
        "  close(): Promise<void> {\n    if (this.closePromise",
        "  close(): Promise<void> {\n    this.clearSettler();\n    if (this.closePromise",
    )
    s = replace_once(
        s,
        "  private fail(reason: Error): void {",
        "  private fail(reason: Error): void {\n    this.clearSettler();",
    )
    return s
