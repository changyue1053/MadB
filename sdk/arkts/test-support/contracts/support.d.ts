/** TEST SUPPORT ONLY; copied into the one combined actual native declaration input. */
export interface NativeSupport {
  testArmWork(engine: string, phase: number): void;
  testReleaseWork(engine: string): void;
  testHoldPublication(engine: string, stream: number, value: number, text: string): void;
  testReleasePublication(engine: string, commit: number): void;
  testFailClone(engine: string, nth: number, mode: number): void;
  testFailCompletion(engine: string, stage: number): void;
  testObserve(runtime: string): number[];
  testExportKey(runtime: string): string;
  testInspect(key: string): number[];
  testObserveOperation(operation: string): number[];
  testArmPartialOpenFailure(): void;
  testLastAdmissionAudit(): number[];
  testDeliveryGate(subscription: string, command: number): number[];
  testHandleAdmission(engine: string, scenario: number, foreignObservationKey: string): void;
  testBootstrapTotals(): number[];
  testMemory(runtime: string): number[];
}
