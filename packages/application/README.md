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

Rendering: the stage is a real DOM tree (CSS handles backdrop, radius, shadow, 3D
transforms, device frames), and export snapshots that node with `modern-screenshot`, so
what you see is exactly what exports, at any pixel ratio.

Editing affordances that must never end up in an export (rulers, grid, and any future
selection handles) carry a `data-export-ignore` attribute. `exportFilter` drops them in
both the image path (`domToBlob`) and the video path (`domToCanvas`), so guides cannot
leak into a PNG or an MP4 by construction. Use that attribute for anything new.

### Credit

This editor is a **Svelte port of [Screenshot Studio](https://github.com/KartikLabhshetwar/screenshot-studio)**
by Kartik Labhshetwar (Apache-2.0). Their product is genuinely good and it shaped both what
this does and how it looks. See [`NOTICE.md`](./NOTICE.md) for the attribution, the license
copy, and the statement of changes.

### Not yet ported

Known gaps against the reference app, so nobody has to rediscover them:

- **Background library.** The upstream R2-hosted galleries (Abstract / macOS / Radiant /
  Raycast / Paper categories, ~100 magic gradients, the Light & Shadow overlays) are not
  here. Deliberate: they need hosted assets, and the plan is to deliver them as Tier-1
  asset packs. We ship a built-in gradient / mesh / pattern / solid set instead.
- **Image overlays ("stickers") and the 3D-objects grid.** Needs an image-overlay type on
  the state; the Layers tab only knows about text and shape overlays today.
- **Annotate tools.** We have 3 (rectangle, ellipse, arrow); upstream has 6 (adds
  curved-arrow, line, and blur regions).
- **Code-snippet and tweet-import cards.** Dropped on purpose: one needs a syntax
  highlighter dependency, the other needs a server endpoint.
- **Layer properties.** Rotation and size are exposed; opacity, blur, and flip are not
  (the overlay types have no such fields yet).
- **Counts.** 28 motion presets vs upstream's 33; our own 3D preset grid rather than their
  40-preset transforms gallery.
- **Timeline.** One clip, one animation track. No multi-clip and no video track.
- **Mobile.** The upstream mobile `Sheet` layout is not ported; the panels are desktop-first.

Preset values (shadow, style-frame, defaults) are pinned against hand-written upstream
fixtures in `apps/desktop/src/lib/screenshot-editor-parity.test.ts`, so drift fails the
test run rather than quietly changing how exports look.
