/** Private C07 composition input, actually consumed by ArkTS. No public wire schema. */
export interface NativeCopy { status: number; value: number; text: string; }
export type Dispatcher = (kind: number, handle: string, copy: NativeCopy | null) => void;
declare const gateway: {
  runtimeOpen(expectedIdentity: string, dispatcher: Dispatcher): string;
  connect(runtime: string): string;
  roundtrip(engine: string, value: number, text: string): NativeCopy;
  submit(engine: string, value: number, text: string): string;
  cancel(operation: string): void;
  observe(engine: string, stream: number): string;
  publish(engine: string, stream: number, value: number, text: string): void;
  requestClose(target: string): void;
  claimResult(operation: string, observationId: string): void;
  watchClose(target: string, observationId: string): number;
  tryTake(runtime: string, observationId: string): NativeCopy | null;
  deliver(subscription: string): number;
  requestRuntimeClose(runtime: string): void;
  pollRuntime(runtime: string): number[];
  consumeRuntimeReceipt(runtime: string): number[];
  audit(runtime: string): number[];
};
export default gateway;
