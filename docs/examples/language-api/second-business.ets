// Independent value-only business input. The current C07 profile rejects it.
// This is not a generated SDK or a claim of a new supported profile.
export interface Reading { value: number; label: string; }

export function normalize(value: Reading): Reading {
  return { value: value.value, label: value.label.trim() };
}
