# `@recast/icons`

Curated icon barrel for the Recast monorepo. Two layers:

- **`@recast/icons`** (root) — Tabler icons re-exported under their
  Lucide-compatible PascalCase names. This is the 90% surface; call sites
  import exactly the way they used to under `@lucide/svelte`.

- **`@recast/icons/ai`** — Phosphor **duotone** accents, **only** for
  AI-touchpoint surfaces (Generate, Re-run, Smart Auto-Zoom, etc.). The
  dual-tone fill/stroke layering signals "this is a smart / AI action"
  more deliberately than a flat outline would. Stay out of this barrel
  for decorative use; the touchpoints are curated.

## Usage

```svelte
<script lang="ts">
  import { Home, Play, MousePointer } from "@recast/icons";
  import { AiWand } from "@recast/icons/ai";
</script>

<Home class="size-4" />
<Play size={16} class="text-primary" />
<AiWand size={14} /> <!-- AI accent -->
```

## API

### Tabler layer (`@recast/icons`)

Each named export is a Svelte component rendered as an inline `<svg>`.
The prop surface accepts:

| Prop     | Type                  | Notes                                                    |
| -------- | --------------------- | -------------------------------------------------------- |
| `size`   | `number \| string`    | Width and height (defaults to 24).                        |
| `stroke` | `number \| string`    | Stroke width (defaults to 2).                            |
| `color`  | `string`              | Applied via `currentColor`-aware strokes and fill.        |
| `class`  | `string`              | Forwarded to the root `<svg>`.                          |
| `aria-*` | `boolean \| "true" \| "false"` | Forwarded as `aria-hidden`/`aria-label`.        |

Tailwind size utilities (`size-4`, `size-3.5`, etc.) win over the `size`
prop via CSS, matching the Lucide ergonomics.

### AI accent layer (`@recast/icons/ai`)

Curated. The current selection:

| Accent   | Phosphor source | Used for                                              |
| -------- | ---------------- | ----------------------------------------------------- |
| `AiWand` | `magic-wand-duotone` | Smart-action affordances: Generate, Re-run, Suggest, auto-focus headers. |
| `AiBrain` | `brain-duotone` | Model-side cues: Smart Auto-Zoom badge, empty states where AI is "thinking". |
| `AiAtom`  | `atom-duotone`   | "Scientific" surface: cursor smoothing, annotation glow. |
| `AiRobot` | `robot-duotone`  | Reserved for the AI assistant panel header. |
| `AiMagic` | `magic-sparkles-duotone` | Reserved alt for smart-action affordances. |
| `AiShine` | `star-spark-duotone` | Reserved alt for highlight chips. |

All AI accents vendored from `@phosphor-icons/core/duotone/<name>.svg`
into `packages/icons/src/ai/<Name>.svelte` to avoid any runtime SVG
parsing and to keep the bundle a single, static asset per accent.

## Why Tabler and not Lucide?

Coverage. Tabler's `6,146`-icon manifest leaves no gaps in the 256
icons Recast uses. Phosphor (the previous alt) only has `~1,300`, so
shipping Tabler primary and Phosphor accents-only is the smallest,
most consistent icon stack.

## Migration

If you're bringing a new icon into the codebase:

1. Look up the icon name on https://tabler.io/icons — Tabler uses
   kebab-case file names (`paint-filled`) but PascalCase exports
   (`IconPaintFilled`).
2. Add the entry to `aliases` in `scripts/icons/generate-tabler-barrel.mjs`.
3. Run `node scripts/icons/generate-tabler-barrel.mjs` — the generated
   `packages/icons/src/tabler/index.ts` re-exports the new icon under
   its Lucide name.
4. Import from `@recast/icons` (not from `@tabler/icons-svelte`).
   Biome will block the latter.

## Anti-patterns

- ❌ Direct `@lucide/svelte`, `@tabler/icons-svelte`, `@phosphor-icons/*`
  imports — blocked by `biome.json`'s `noRestrictedImports`.
- ❌ Decorative use of `AiWand`/`AiBrain`/`AiAtom` outside AI touchpoints.
- ❌ Adding new icons without updating `aliases` — the codemod only
  re-uses names that already exist in the barrel.

## Hand-stitched fallbacks

A small set of icons have no direct Tabler equivalent (sparse Lucide
glyphs). These live under `packages/icons/src/fallback/` and are
re-exported through the barrel — see the file comments there for the
visual rationale.