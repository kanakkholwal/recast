// Public surface of @recast/icons.
//
// Consumers should import named icons from the root:
//   import { Home, Play, Sparkles } from "@recast/icons";
//
// Subpath imports are reserved for advanced use (e.g. tree-shake audits):
//   import { Home, Play } from "@recast/icons/tabler";
//   import { AiSparkle } from "@recast/icons/ai";
//
// All icons are re-exported under Lucide-compatible PascalCase names so call
// sites only need to swap `from "@recast/icons"` for `from "@recast/icons"`
// — no per-call rewrite. The codemod in `scripts/icons/codemod.mjs` is the
// single point where Tabler's small API differences (`stroke` vs
// `strokeWidth`, `XFilled` for filled variants) get translated.

export type { ClassValue } from "clsx";
export { cn } from "./utils";
export * from "./types";
export * from "./tabler/index";
export * from "./ai/index";
