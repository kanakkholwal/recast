# Contributing to Recast

First off, thank you for considering contributing to Recast! It's people like you that make open-source software such a great community to learn, inspire, and create.

## 🧠 Codebase Mental Model

Recast is a highly-optimized monorepo relying on four core technical pillars:

1. **Tauri & Rust:** All OS-level APIs, audio/video device management (camera, microphone, screens), cursor tracking, and high-performance FFmpeg rendering are natively handled in `apps/desktop/src-tauri/`.
2. **Svelte 5 (Runes):** The UI layer explicitly targets modern Svelte 5 paradigms (`$state`, `$derived`, `$effect`). Please avoid older Svelte 4 reactivity patterns (`export let`, `$:`) during development.
3. **Tailwind CSS v4:** We use bleeding-edge v4 utility structures. Global design scopes, variables, and aesthetic configurations are managed in `packages/design` using the `@theme` directive rather than a monolithic `tailwind.config.js`.
4. **Turborepo:** We strictly isolate the core `apps/` from the internal toolkits `packages/` to ensure modularity, fast building, and cache retention via `turbo.json`.

The workspace is a **pnpm** workspace built with **Turborepo** for fast, cached builds:

| Package | Path | Description |
|---|---|---|
| **🏔 Recast Desktop** | `apps/desktop` | The core product — a Tauri + Rust backend with a SvelteKit + Svelte 5 frontend editor. |
| **🌐 Recast Web** | `apps/web` | The marketing landing page and distribution site, built on SvelteKit. |
| **🧩 UI Library** | `packages/ui` | Headless, accessible internal Svelte UI component library. |
| **🎨 Design Tokens** | `packages/design` | Centralized Tailwind CSS v4 design scope and typographic assets. |

## 🛠 Local Development Setup

### Quick start (recommended)

We ship a one-shot setup script that bootstraps everything a contributor
needs to build and run the desktop app. It detects your OS and architecture,
auto-installs the missing toolchains (Node.js, pnpm, Rust, and the Tauri
OS-level prerequisites), downloads the FFmpeg + ffprobe sidecar binaries into
`apps/desktop/src-tauri/binaries/`, installs workspace dependencies, and
produces a debug build to verify the toolchain.

```bash
git clone https://github.com/kanakkholwal/recast.git
cd recast
```

**Windows** (PowerShell):

```powershell
powershell -ExecutionPolicy Bypass -File scripts/setup.ps1
```

**macOS / Linux**:

```bash
bash scripts/setup.sh
```

Once Node.js is installed you can also re-run it via `pnpm setup:ffmpeg`.
Useful flags (forward with `pnpm setup:ffmpeg -- <flag>`):

- `--skip-build` / `-SkipBuild` — stop after install + FFmpeg download.
- `--skip-toolchains` / `-SkipToolchains` — only verify toolchains, don't install.

When it finishes, start the desktop app with hot-reloading:

```bash
pnpm --filter recast-desktop dev
```

### Manual setup

If you'd rather set things up by hand, or the script doesn't cover your
environment:

1. Install **Node.js** (v18+) and **pnpm** (v9+).
2. Install **Rust** (v1.70+) and [Tauri OS-Specific Prerequisites](https://v2.tauri.app/start/prerequisites/) (such as C++ build tools for Windows or webkit2gtk for Linux).
3. Provide the **FFmpeg sidecar binaries** — see
   [`apps/desktop/src-tauri/binaries/README.md`](apps/desktop/src-tauri/binaries/README.md)
   for the required target-triple file names. These are gitignored because of
   their size, so each contributor downloads them locally.
4. Install dependencies and start the dev server:
   ```bash
   pnpm install
   pnpm turbo run dev --filter=recast-desktop
   ```

## 📦 Building for Production

To build distributable binaries for your current platform:

```bash
pnpm turbo run build --filter=recast-desktop
```

This produces installers (`.msi` / `.nsis` on Windows, `.dmg` on macOS,
`.AppImage` / `.deb` on Linux) under `apps/desktop/src-tauri/target/release/`.

## 🖼 Wallpaper Assets (external download)

No wallpaper imagery — neither full-res PNGs nor WebP thumbnails — ships with
the installer. Everything is distributed via a **separate GitHub Release**
(tagged like `wallpapers-v1`) and downloaded on first launch into the user's
app data dir. Thumbs download first (a few KB each, seconds); full-res PNGs
follow in the background. All files are SHA-256-verified against
`manifest.json` before they're written to disk.

**Release layout — two independent tags:**

| Tag              | Assets uploaded                                      |
|------------------|------------------------------------------------------|
| `v0.0.1`, `v0.0.2`, … | App installers (`.msi`, `.dmg`, `.AppImage`, `latest.json`) |
| `wallpapers-v1`, `wallpapers-v2`, … | `*.png` + `*.webp` + `manifest.json` |

This keeps app updates small (no wallpaper re-downloads) and lets you refresh
wallpapers without cutting a new app version.

**Fallback behaviour:**

- **First-run online** — picker shows a CSS gradient placeholder; thumbs land within seconds; full-res replaces the thumb once downloaded.
- **First-run offline** — CSS placeholder + "Offline" badge on every tile; no crash; downloader retries automatically when `window.online` fires.
- **Subsequent runs offline** — hydrates instantly from the on-disk `manifest.lock.json`, no network call needed.
- **Corruption** — mismatched SHA-256 triggers re-download on next install pass.

**To add / update wallpapers:**

1. Drop the PNG(s) into `assets/backgrounds/wallpapers/` and commit.
2. Tag and push — the [Release Wallpaper Assets](.github/workflows/release-wallpapers.yml)
   workflow regenerates thumbs + manifest and publishes the release:
   ```sh
   git tag wallpapers-v2
   git push origin wallpapers-v2
   ```
   Or trigger it manually from the Actions tab with a tag input.

   To run locally instead:
   ```sh
   RELEASE_TAG=wallpapers-v2 pnpm prepare:assets-wallpapers
   gh release create wallpapers-v2 \
     ./assets/backgrounds/wallpapers/*.png \
     ./assets/backgrounds/thumbs/*.webp \
     ./assets/manifest.json
   ```
3. Point the app at the new release by setting `PUBLIC_ASSETS_MANIFEST_URL`
   at build time (or bumping the default in [apps/desktop/src/lib/assets.ts](apps/desktop/src/lib/assets.ts)).
4. If adding a new wallpaper id, extend `WALLPAPERS` in
   [apps/desktop/src/lib/stores/editor-store.svelte.ts](apps/desktop/src/lib/stores/editor-store.svelte.ts).

**Cache location** (delete to force a full re-download):

- Windows: `%APPDATA%\com.nexonauts.recast\assets\`
- macOS: `~/Library/Application Support/com.nexonauts.recast/assets/`
- Linux: `~/.local/share/com.nexonauts.recast/assets/`

The folder contains each downloaded file plus a `manifest.lock.json` — the
persisted snapshot used for offline hydration.

## 📝 Pull Request Process

1. **Keep it focused:** Ensure your PR attempts to do one thing specific and well. Whether it's adding a singular feature or fixing a specific bug, please avoid massive, monolithic PRs.
2. **Discuss architecture first:** If you want to add a heavy new Rust crate, alter the Svelte 5 state management, or introduce a new internal package to the monorepo workspace, please open an Issue to discuss it with the maintainers first to avoid wasted effort.
3. **Follow the Craft aesthetic:** Any UI additions should utilize our internal shared components from `packages/ui` and respect the "Smooth by Default", minimalist, glassmorphism visual language defining the product.
4. **Lint and Standardize:** Before submitting a PR, ensure your branch passes its build pipeline entirely:
   ```bash
   pnpm turbo run build
   # Run standard linters and spellcheck here
   ```

## 📰 Changelog & Release Notes

Every user-visible change ships with a **changeset** — a small markdown file
in `.changeset/` describing what changed and how big the bump is. We use these
to assemble `CHANGELOG.md`, the GitHub release body, the desktop app's
"What's new" panel, and the web `/changelog` page from a single source.

### When to add one

| Change                                                        | Changeset? |
| ------------------------------------------------------------- | ---------- |
| New feature, behavior change, or bug fix users will notice    | ✅ Yes     |
| Performance / UX polish that's worth advertising              | ✅ Yes     |
| Refactor, internal tooling, dependency bumps, test-only edits | ❌ No      |
| Docs-only edits (this file, READMEs)                          | ❌ No      |

When in doubt, add one.

### How to add one

From the repo root, run:

```bash
pnpm changeset
```

The interactive prompt asks:

1. **Which package?** Pick `recast-desktop`. The other workspace packages
   (`recast-web`, `@recast/design`, `@recast/ui`) are intentionally ignored
   in `.changeset/config.json` — they're not separately released.
2. **Bump kind?** `patch` for fixes, `minor` for new features, `major` for
   breaking changes (rare in beta).
3. **Summary?** One sentence in the imperative present tense, written for an
   end user — _"Region picker buttons work again"_, not _"Fix isVisible
   mutation in RegionPicker.svelte"_.

That generates a file like `.changeset/short-pandas-dance.md`. **Open it and
add a `kind:` line** so the release script knows which Keep-a-Changelog
section the entry belongs in:

```markdown
---
"recast-desktop": minor
kind: added
---

Active-preset chip in the editor toolbar with reset-to-source.
```

`kind` is one of `added`, `changed`, `fixed`, `deprecated`. Default is
`changed` if you forget — better to set it explicitly.

### Tips for good entries

- **Write for users, not reviewers.** _"GIF export progress now advances in
  real time"_ beats _"Switch GIF pipeline to 2-pass palettegen"_.
- **One sentence, no period soup.** Backticks for code identifiers, em-dashes
  for asides, no trailing "in this PR".
- **One change per changeset.** If your PR ships two distinct things, add two
  files via `pnpm changeset` twice.
- **Highlights are optional.** If a release deserves a top-of-section
  callout, add a `### Highlights` block manually in `CHANGELOG.md` after the
  release is cut — the desktop dialog renders it as the punchy bullet row.

### What happens to your changeset

You don't manage versions or `CHANGELOG.md` directly — the maintainer runs
`pnpm release:prepare <version>` periodically, which:

1. Consumes every `.changeset/*.md`,
2. Merges them into a new `## [<version>] — <date>` section in
   `CHANGELOG.md`,
3. Regenerates `apps/desktop/src/constants/changelog.ts` so the desktop
   app's "What's new" panel reflects the section,
4. Deletes the consumed changeset files.

Once the resulting commit is tagged `vX.Y.Z`, the
[`Release Desktop App`](.github/workflows/release-desktop.yml) workflow
builds artifacts, uses your changeset's section as the GitHub release body,
and the web `/changelog` page picks it up from the GitHub Releases API.

For the maintainer-side details, see
[`.changeset/README.md`](.changeset/README.md).

## 📋 Reporting Bugs

When logging a bug in our GitHub Issue tracker, please include:
- Your Operating System and its exact version (e.g., macOS Sonoma 14.2, Windows 11 23H2).
- The exact steps required to reproduce the issue.
- Contextual screen recordings or logs pulled directly from the Tauri compiler outputs.