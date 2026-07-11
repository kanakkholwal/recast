# @recast/application

Heavy, application-level composite features shared by `apps/web` and `apps/desktop`.

`@recast/ui` stays lean: base design-system primitives (button, input, dialog, slider,
color-picker, …). Anything large, stateful, or feature-shaped lives here instead, so the
primitive layer never carries a whole editor's weight.

Source-shipping package (same pattern as `@recast/player`): the app compiles our `src`
directly, so consuming apps must list our `src` in their Tailwind `@source` and import our
stylesheet once at the app entry.

## First tenant: Screenshot Editor

`@recast/application/screenshot-editor` — take a screenshot (native capture on desktop, or
upload/paste/drop anywhere) and beautify it: backdrop, padding, rounded corners, shadow,
aspect-ratio presets. Export to PNG/JPG at up to 4x or copy straight to the clipboard.

```svelte
<script lang="ts">
  import { ScreenshotEditor } from "@recast/application/screenshot-editor";
  // Desktop passes a native-capture callback; the web omits it (upload/paste only).
</script>

<ScreenshotEditor oncapture={captureFromDisplay} />
```

Consuming apps must:

1. Add `"@recast/application": "workspace:*"` to `dependencies`.
2. Add `@source "../node_modules/@recast/application/src";` to `src/app.css`
   (else the editor's utility classes purge in release builds only — see
   `tailwind_source_workspace_pkgs`).
3. `import "@recast/application/styles.css";` once in the root `+layout.svelte`.

Rendering: the stage is a real DOM tree (CSS handles backdrop, radius, shadow, and later
3D transforms / device frames), and export snapshots that node with `modern-screenshot`,
so what you see is exactly what exports, at any pixel ratio.
