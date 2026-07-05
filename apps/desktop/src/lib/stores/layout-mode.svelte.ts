import { PersistedState } from "@recast/ui/persisted-state";

export type LayoutMode = "os-native" | "recast";

/** Option metadata for the Settings → General segmented control. */
export const LAYOUT_MODES: {
  value: LayoutMode;
  label: string;
  hint: string;
}[] = [
  {
    value: "os-native",
    label: "OS native",
    hint: "Window controls follow your operating system: traffic lights on the left on macOS, minimize, maximize, and close on the right on Windows and Linux.",
  },
  {
    value: "recast",
    label: "Recast",
    hint: "Recast's own unified titlebar, identical on every OS.",
  },
];

/**
 * App-shell chrome preference. Pure UI state, shared via `PersistedState` so the
 * Settings toggle and the `(app)` shell react to the same value; persists in
 * localStorage and broadcasts across same-origin windows.
 */
export const layoutMode = new PersistedState<LayoutMode>(
  "recast-layout-mode",
  "os-native",
);
