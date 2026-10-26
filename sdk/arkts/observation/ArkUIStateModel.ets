// Optional ArkUI V2 entry. Install beside StateObservation and Boundary.
import { Engine, Snapshot } from './Boundary';
import { ObservationPhase, StateObservation } from './StateObservation';

@ObservedV2
export class ArkUIStateModel {
  @Trace snapshot: Snapshot | undefined = undefined;
  private readonly observation: StateObservation = new StateObservation();

  get phase(): ObservationPhase { return this.observation.phase; }
  get closeFailure(): Error | undefined { return this.observation.closeFailure; }

  attach(engine: Engine): void {
    let admitted: boolean = false;
    let first: Snapshot | undefined = undefined;
    this.observation.attach(engine, (value: Snapshot) => {
      if (admitted) { this.snapshot = value; }
      else { first = value; }
    });
    admitted = true;
    // Synchronous registration may deliver and then fail. Publish only after
    // admission succeeds; preserve its latest real value, or clear old state.
    const initial: Snapshot | undefined = first;
    first = undefined;
    this.snapshot = initial;
  }

  requestStop(): void { this.observation.requestStop(); }
  close(): Promise<void> { return this.observation.close(); }
}
