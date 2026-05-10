# Contributing to Recast

First off, thank you for considering contributing to Recast! It's people like you that make open-source software such a great community to learn, inspire, and create.

## 🧠 Codebase Mental Model

Recast is a highly-optimized monorepo relying on four core technical pillars:

1. **Tauri & Rust:** All OS-level APIs, audio/video device management (camera, microphone, screens), cursor tracking, and high-performance FFmpeg rendering are natively handled in `apps/desktop/src-tauri/`.
2. **Svelte 5 (Runes):** The UI layer explicitly targets modern Svelte 5 paradigms (`$state`, `$derived`, `$effect`). Please avoid older Svelte 4 reactivity patterns (`export let`, `$:`) during development.
3. **Tailwind CSS v4:** We use bleeding-edge v4 utility structures. Global design scopes, variables, and aesthetic configurations are managed in `packages/design` using the `@theme` directive rather than a monolithic `tailwind.config.js`.
4. **Turborepo:** We strictly isolate the core `apps/` from the internal toolkits `packages/` to ensure modularity, fast building, and cache retention via `turbo.json`.

## 🛠 Local Development Setup

To work on Recast locally, please make sure you've fulfilled the following prerequisites:

1. Install **Node.js** (v18+) and **pnpm** (v9+).
2. Install **Rust** (v1.70+) and [Tauri OS-Specific Prerequisites](https://v2.tauri.app/start/prerequisites/) (such as C++ build tools for Windows or webkit2gtk for Linux).
3. Clone the repo and install dependencies:
   ```bash
   git clone https://github.com/kanakkholwal/recast.git
   cd recast
   pnpm install
   ```
4. Start a development server. For instance, to work on the desktop app with hot-reloading:
   ```bash
   pnpm turbo run dev --filter=recast-desktop
   ```

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