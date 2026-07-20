# `@recast/media`

Shared media-processing layer for Recast. Wraps [MediaBunny](https://mediabunny.dev/) and
the modern web platform (WebCodecs, Web Workers, AudioWorklet, IndexedDB, Streams)
behind one opinionated, high-level API consumed by the desktop video editor's
preview pipeline and the web app's conversion tools.

**Design headroom:** built to support a future in-browser video editor for
100 GB+ source files.

## Read these in order

1. **[`REQUIREMENTS.md`](./REQUIREMENTS.md)** — the non-negotiable contract:
   public API surface, performance budgets, curated web.dev guides,
   implementation rules, browser API surface, testing contract.
2. **[`MIGRATION-LOG.md`](./MIGRATION-LOG.md)** — what shipped, PR by PR, and
   the defects found auditing it. Read PR-G onward before trusting any claim
   that a part of this package is "done".
3. **AGENTS.md** (root) §2 rule 12 and §4.x — the project-wide rules this
   package enforces.

## Consumers

- `apps/desktop/src/routes/editor/[file]/` — primary: video editor preview pipeline.
- `apps/web/src/routes/tools/*` — browser conversion tools (trim, mute,
  compress, resize, transcode, extract-audio, video-to-gif, audio-to-mp3,
  extract-frames).
- **Future:** in-browser editor for very large source files.

## Out of scope

- The Rust export pipeline (`apps/desktop/src-tauri/src/commands/export/*`)
  stays as-is. The `EnqueueExportRequest` IPC payload is byte-stable.
- The web app's screenshot editor
  (`packages/application/src/screenshot-editor/*`) keeps its `mp4-muxer`
  legacy path.
- `apps/desktop/src/lib/timeline/filmstrip-*` is a separate hot path;
  revisit later.

## Status

Pre-PR-A. Scaffolding + requirements doc land together.
