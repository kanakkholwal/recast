// AI-touchpoint accents — Phosphor duotone.
//
// This barrel is curated. The selection lives in the per-file components
// (AiWand.svelte, AiBrain.svelte, ...); only the ones this file
// re-exports become available under `@recast/icons/ai`. To add a new
// accent: (1) drop a `phosphor-duotone-<name>.svg` under `./assets/` and
// (2) author an `<Ai>.svelte` that consumes it, then (3) add the export
// below.
//
// See AGENTS.md §4 (icon policy) for the rules these accents follow.

export { default as AiWand } from "./AiWand.svelte";
export { default as AiBrain } from "./AiBrain.svelte";
export { default as AiRobot } from "./AiRobot.svelte";
export { default as AiMagic } from "./AiMagic.svelte";
export { default as AiAtom } from "./AiAtom.svelte";
export { default as AiShine } from "./AiShine.svelte";
