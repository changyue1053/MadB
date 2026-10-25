export const FACADE_FRAME_LENGTH: number = 15;
export enum FacadeFrame {
  Format = 0,
  WaiterMap = 1,
  SettlingWaiters = 2,
  SubscriptionsMap = 3,
  OperationsMap = 4,
  ParentCloseControl = 5,
  TimerArmed = 6,
  TickInFlight = 7,
  Closing = 8,
  Closed = 9,
  Failed = 10,
  AcceptedResolveReturned = 11,
  AcceptedRejectReturned = 12,
  SettlementProbeArmed = 13,
  SettlementFaultArmed = 14,
}
