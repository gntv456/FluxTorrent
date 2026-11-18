export const DENSITY_STORAGE_KEY = "flux-density";
export type UiDensity = "comfortable" | "compact";

export function isUiDensity(v: unknown): v is UiDensity {
  return v === "comfortable" || v === "compact";
}
