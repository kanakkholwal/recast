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

## 📋 Reporting Bugs

When logging a bug in our GitHub Issue tracker, please include:
- Your Operating System and its exact version (e.g., macOS Sonoma 14.2, Windows 11 23H2).
- The exact steps required to reproduce the issue.
- Contextual screen recordings or logs pulled directly from the Tauri compiler outputs.