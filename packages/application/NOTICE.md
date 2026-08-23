# Third-party notices

## Screenshot Studio

`@recast/application/screenshot-editor` is a **Svelte port** of
[Screenshot Studio](https://github.com/KartikLabhshetwar/screenshot-studio) by
**Kartik Labhshetwar**. We loved the product, and it shaped both the feature set and the
interface of our editor. Credit where it is due.

```
Copyright 2025 Kartik Labhshetwar
Licensed under the Apache License, Version 2.0
```

- Upstream: https://github.com/KartikLabhshetwar/screenshot-studio
- Full license text: [`licenses/screenshot-studio-Apache-2.0.txt`](./licenses/screenshot-studio-Apache-2.0.txt)
- You may also obtain the license at http://www.apache.org/licenses/LICENSE-2.0

### Statement of changes

As required by Section 4(b) of the Apache License, we note that this is a derivative work
and the files under `src/screenshot-editor/` have been **modified** from the original.
It is a reimplementation, not a copy: no upstream source file is used verbatim.

What we changed:

- **Rewritten from React to Svelte 5.** The original is a Next.js / React app using Zustand
  (with `zundo` for history) and Radix primitives. Ours is Svelte 5 runes (a `.svelte.ts`
  state class with its own coalescing undo/redo) built on our `@recast/ui` components.
- **Rendering is DOM-first, not Konva.** The original composites on a Konva canvas with a
  mirrored store. Our stage is a real DOM tree, and export snapshots that node with
  `modern-screenshot`, so the preview and the export are the same thing by construction.
- **Different feature surface.** We do not port the tweet import, the code-snippet card, the
  photoreal device-frame mockups (their PNGs are served remotely upstream), or a multi-track
  keyframe timeline. We add a `data-export-ignore` mechanism so editing guides can never be
  baked into an export, an MP4 path built on WebCodecs + `mp4-muxer`, and draft autosave.
- **Preset values were transcribed, not copied.** The gradient/mesh/solid/magic background
  tables, aspect ratios, the font library, and the shadow/style-frame/default tables mirror
  the upstream values so the output looks right; they are transcribed into
  `src/screenshot-editor/{backgrounds-data,fonts,defaults}.ts` and exercised by the package's
  `render`/`animation`/`export` unit tests.

### Bundled image assets

We bundle a subset of the upstream image backgrounds and light/shadow overlays (the
`radiant`, `mesh`, `pattern`, `paper`, and `overlay-shadow` sets). They live once in
`packages/application/assets/screenshot-assets/` and are copied into each consuming
app's `static/screenshot-assets/` at dev/build time (`scripts/sync-screenshot-assets.mjs`;
the generated copies are gitignored so the repo keeps a single copy). These originate
from the Screenshot Studio repository
(Apache-2.0). We deliberately **do not** bundle the upstream `mac` (Apple) or `raycast`
(Raycast) wallpaper packs, nor the large photographic `assets` set, because those are
third-party works with their own copyrights; they remain out of this distribution.

### Licensing

Recast as a whole is dual-licensed (GPLv3 / commercial) per the repository `LICENSE.md`.
Apache-2.0 is one-way compatible with GPLv3, so the ported work may be distributed under
Recast's terms, provided this notice and the license copy above travel with it.
