# Services layer

Orchestration that spans the editor store + Tauri IPC + browser-side
rasterization. A service takes a project (`EditorStore`) plus parameters and
performs a complete operation: building an export payload, running an export,
analyzing a recording for auto-zoom, etc.

This is the **headless-core** layer. Both the Svelte UI and a future in-app MCP
server call these same functions as thin clients, so an agent can drive the
editor through the exact code paths the GUI uses.

```
  Svelte components (editor route)        MCP server (future)
                 \                       /
                  \                     /
            ┌──────────────────────────────────┐
            │  services/  (orchestration)       │  ← you are here
            │  buildExportRenderState, runExport│
            │  generateAutoZoom                 │
            ├──────────────────────────────────┤
            │  store mutation methods (actions) │  addZoomRegion, addCut, …
            ├──────────────────────────────────┤
            │  EditorRenderState (the document) │  serializable project state
            ├──────────────────────────────────┤
            │  pure logic: eval / easing / etc. │
            ├──────────────────────────────────┤
            │  lib/ipc.ts → Rust commands       │
            └──────────────────────────────────┘
```

## Rules for code in here

- **No UI state.** No toasts, progress rings, dialog phases, or run-guards.
  Surface progress through optional callbacks/hooks; let the caller own its UI.
- **No DOM/component coupling.** Take an `EditorStore` and plain params; return
  data or a result. Anything a component needs to *display* stays in the
  component.
- **Decouple from UI singletons.** Pass feature flags in (e.g.
  `silenceDetectionEnabled`) rather than importing `experimentalStore` here.
- **Serializable in, serializable out** where possible. That's what makes an
  operation callable by an agent.

## Convention for the rest of the app

Routes and components own only UI state (selection, hover, panel visibility,
media-element wiring) and call into services (orchestration) or the store's
mutation methods (the **actions** contract) for everything else. Domain logic
does not live in `.svelte` files.

## Current services

- `export.ts`: `buildExportRenderState` (hybrid-raster + per-lane toggles →
  the exact `EditorRenderState` Rust renders) and `runExport` (progress-listener
  lifecycle + `exportVideo`).
- `analysis.ts`: `generateAutoZoom` (suggest focus regions from the cursor
  track, place them, persist).

Verify changes with `pnpm --filter ./apps/desktop run check`.
