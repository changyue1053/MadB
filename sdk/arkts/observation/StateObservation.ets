// Optional C07 source extension. Install beside the generated Boundary.ets.
import { Engine, Snapshot, Subscription } from './Boundary';

export type ObservationPhase = 'idle' | 'attaching' | 'attached' | 'stopping' | 'closing' | 'closeFailed';

export class StateObservationError extends Error implements Error {
  readonly code: string;
  constructor(code: string) { super(code); this.name = 'StateObservationError'; this.code = code; }
}

class Failure {
  readonly reason: Error;
  constructor(reason: Error) { this.reason = reason; }
}

class Attachment {
  accepting: boolean = true;
  listener: ((value: Snapshot) => void) | undefined;
  subscription: Subscription | undefined = undefined;
  registrationFailure: Failure | undefined = undefined;
  closeAttempt: Promise<void> | undefined = undefined;

  constructor(listener: (value: Snapshot) => void) { this.listener = listener; }
}

// All calls and deliveries use the Engine's owning env. This class neither
// dispatches across threads nor owns the Engine/Runtime or a payload queue.
export class StateObservation {
  private attachment: Attachment | undefined = undefined;
  private phaseValue: ObservationPhase = 'idle';
  private closeFailureValue: Error | undefined = undefined;

  get phase(): ObservationPhase { return this.phaseValue; }
  get closeFailure(): Error | undefined { return this.closeFailureValue; }

  attach(engine: Engine, listener: (value: Snapshot) => void): void {
    if (this.attachment !== undefined) { throw new StateObservationError('Busy'); }
    if (typeof listener !== 'function') { throw new StateObservationError('InvalidArgument'); }
    const attachment: Attachment = new Attachment(listener);
    this.attachment = attachment;
    this.phaseValue = 'attaching';
    this.closeFailureValue = undefined;
    try {
      attachment.subscription = engine.observeState((value: Snapshot) => { this.deliver(attachment, value); });
    } catch (error) {
      // Registration did not return a Subscription. Its rollback belongs to
      // Engine.observeState, as in the ordinary API, not to a guessed handle.
      attachment.registrationFailure = new Failure(error as Error);
      this.stopDelivery(attachment);
      if (attachment.closeAttempt === undefined) { this.attachment = undefined; this.phaseValue = 'idle'; }
      throw error as Error;
    }
    if (attachment.accepting) {
      this.phaseValue = 'attached';
    } else {
      // A synchronous receiver may stop/close before observeState returns.
      // Keep the acquired subscription even when its stop request throws.
      attachment.subscription.requestStop();
    }
  }

  requestStop(): void {
    const attachment: Attachment | undefined = this.attachment;
    if (attachment === undefined) { return; }
    this.stopDelivery(attachment);
    if (this.phaseValue !== 'closing' && this.phaseValue !== 'closeFailed') { this.phaseValue = 'stopping'; }
    const subscription: Subscription | undefined = attachment.subscription;
    if (subscription !== undefined) { subscription.requestStop(); }
  }

  close(): Promise<void> {
    const attachment: Attachment | undefined = this.attachment;
    if (attachment === undefined) { return Promise.resolve(); }
    if (attachment.closeAttempt !== undefined) { return attachment.closeAttempt; }
    this.stopDelivery(attachment);
    this.phaseValue = 'closing';
    this.closeFailureValue = undefined;
    let requestFailure: Failure | undefined = undefined;
    // Register the shared attempt before calling outside code. The microtask
    // also lets a synchronous observeState invocation return its subscription.
    const attempt: Promise<void> = Promise.resolve().then((): Promise<void> => {
      if (requestFailure !== undefined) { return Promise.reject(requestFailure.reason); }
      if (attachment.registrationFailure !== undefined) { return Promise.reject(attachment.registrationFailure.reason); }
      const subscription: Subscription | undefined = attachment.subscription;
      if (subscription === undefined) { return Promise.reject(new StateObservationError('MissingSubscription')); }
      return subscription.close();
    }).then(() => {
      attachment.subscription = undefined;
      attachment.closeAttempt = undefined;
      this.attachment = undefined;
      this.phaseValue = 'idle';
      this.closeFailureValue = undefined;
    }, (error: Error) => {
      attachment.closeAttempt = undefined;
      this.closeFailureValue = error;
      if (attachment.subscription === undefined) {
        // Failed registration returned no resource for this adapter to retain.
        this.attachment = undefined;
        this.phaseValue = 'idle';
      } else { this.phaseValue = 'closeFailed'; }
      throw error;
    });
    attachment.closeAttempt = attempt;
    try { this.requestStop(); } catch (error) { requestFailure = new Failure(error as Error); }
    return attempt;
  }

  private stopDelivery(attachment: Attachment): void {
    attachment.accepting = false;
    attachment.listener = undefined;
  }

  private deliver(attachment: Attachment, value: Snapshot): void {
    if (this.attachment !== attachment || !attachment.accepting) { return; }
    const listener: ((value: Snapshot) => void) | undefined = attachment.listener;
    if (listener === undefined) { return; }
    try { listener(value); }
    catch (error) {
      this.stopDelivery(attachment);
      if (this.phaseValue !== 'closing') { this.phaseValue = 'stopping'; }
      // The C07 listener boundary owns CallbackFailure. Preserve the user's
      // thrown identity here; a second stop error must not replace it.
      throw error as Error;
    }
  }
}
