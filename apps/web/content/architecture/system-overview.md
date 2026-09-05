---
kind: architecture
title: "System overview"
description: "A Rust recorder, a Svelte editor engine, and one wgpu compositor that draws both the preview and the export."
position: 0
status: production
domain: platform
summary: "One compositor draws the preview and the export, so the two cannot drift."
inputs:
  - "A screen, window, or region selection"
  - "Microphone and system audio"
  - "An optional camera"
outputs:
  - "A .recast project bundle"
  - "An exported .mp4 or .gif"
entrypoints:
  - "apps/desktop/src-tauri/src/lib.rs"
  - "packages/editor/src/components/Editor.svelte"
  - "apps/desktop/src/routes/+layout.svelte"
invariants:
  - "One compositor (the Rust engine, wasm in the browser) serves preview and export, so a visual bug is fixed once."
  - "@recast/editor never imports @tauri-apps; the desktop host injects every native capability."
  - "The @recast/* packages ship source, so each app compiles their .svelte and .ts itself."
---

Recast is an **offline-first desktop screen recorder + video editor**. Stack: **Tauri v2** (Rust backend) + **Svelte 5** (runes) frontend, in a **pnpm monorepo**. The editor engine lives in the `@recast/editor` package; the desktop app (`recast-desktop`) is a thin **host** that wires it to native capabilities through injected services and host-hooks.

The guiding architectural bet: **one compositor** drives *both* the live preview and the export, so the two can never diverge. It is a Rust crate over wgpu, compiled to wasm for the browser and natively for the desktop, and FFmpeg is demoted from a second compositor to a pure **muxer**. Recording is Rust; editing and preview run in the WebView, where the engine is the same crate compiled to wasm; export composites either there with WebCodecs or natively in the same crates, and persistence and heavy native work cross the Tauri IPC boundary.

## Package & host map

```mermaid
graph TD
    desktop["recast-desktop<br/>(Tauri host app)"] -->|imports components| editor["@recast/editor<br/>(editor engine)"]
    desktop <-->|Tauri IPC| rust["Rust core (src-tauri)"]
    desktop -. "setEditorServicesForApp()<br/>setEditorHostHooks()" .-> editor
    editor --> media["@recast/media<br/>(decode + workers)"]
    editor --> captions["@recast/captions"]
    editor --> render["@recast/render"]
    editor --> ui["@recast/ui"]
    editor --> player["@recast/player"]
    web["recast-web"] --> editor
    web --> player
```

The `@recast/*` packages **ship source** (their `exports` map points at `./src`), so each consuming app compiles their `.svelte`/`.ts` itself. This is load-bearing for Vite config (see [04-media-decode-and-workers.md](/architecture/media-decode-workers)).

## Runtime process model

```mermaid
flowchart LR
    subgraph WV["WebView2 (frontend, one per window)"]
        svelte["Svelte UI"]
        gl["wasm engine / WebCodecs"]
        wk["Web Workers ×5<br/>(mediabunny, filmstrip,<br/>render, export, smoothing)"]
        svelte --> gl
        svelte --> wk
    end
    svelte <-->|"invoke() / ipc::Channel events"| core["Rust core"]
    core --> ff["FFmpeg sidecar"]
    core --> sqlite[("SQLite (export queue + index)")]
    core --> fs["filesystem (.recast, temp)"]
```

## Conventions

- **`path:line`** citations point at the real source at time of writing.
- Diagrams are Mermaid, written inline in the markdown so the source of a
  diagram is readable without rendering it.
- "Host" means the desktop app (`apps/desktop`); "package" or "engine" means
  `@recast/editor`.
- Every page states its inputs, outputs, entrypoints, and invariants in
  frontmatter. Those are the summary; the prose is the detail.
