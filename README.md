<h1 align="center">Recast</h1>

<p align="center">
  <strong>Video editing, refined.</strong> The fast, minimal, and intentional editor built for the next generation of storytellers.
</p>

<p align="center">
  <a href="https://github.com/kanakkholwal/recast/actions"><img src="https://img.shields.io/github/actions/workflow/status/kanakkholwal/recast/release-desktop?style=flat-square" alt="Build Status"></a>
  <a href="https://github.com/kanakkholwal/recast/blob/main/LICENSE.md"><img src="https://img.shields.io/badge/license-Dual_License-blue.svg?style=flat-square" alt="License: Dual License (GPLv3 / Commercial)"></a>
</p>

Recast is a high-performance open-source screen recording tool with integrated, cinematic editing capabilities. It aims to replace messy timeline-based tools with a "Smooth by Default" experience for startups and creators generating polished product demos. 

## ✨ Key Features
- **Cinematic Magic by Default:** Perfect cursor motion smoothing, automatic zooming, and intelligent tracking.
- **Zero-Lag Recording:** Built natively with Tauri and Rust, offloading high-performance video encoding (FFmpeg) to the user's silicon.
- **Privacy-First:** Locally generated user profiles without invasive tracking.
- **Sleek Interface:** "Craft" design system featuring minimal glassmorphism, native blurs, and Svelte 5 reactivity.

## 🏗 Architecture (Monorepo)

Recast is structured as a **pnpm** workspace utilizing **Turborepo** for fast, cached builds.

| Package | Path | Description |
|---|---|---|
| **🏔 Recast Desktop** | `apps/desktop` | The core product: A Tauri + Rust backend and a SvelteKit + Svelte 5 frontend editor pane. |
| **🌐 Recast Web** | `apps/web` | The marketing landing page and distribution site, built on SvelteKit. |
| **🧩 UI Library** | `packages/ui` | Headless, accessible internal Svelte UI component library. |
| **🎨 Design Tokens** | `packages/design` | Centralized Tailwind CSS v4 design scope and typographic assets. |

## 📸 Screenshots

![Screenshot 1 Placeholder](assets/screenshots/preview_homescreen.png)
![Screenshot 2 Placeholder](assets/screenshots/preview_profiles.png)

## 🚀 Getting Started

### Prerequisites
- Node.js (v18+)
- [pnpm](https://pnpm.io/) (v9+)
- Rust (v1.70+) & Cargo
- [Tauri Dependencies](https://v2.tauri.app/start/prerequisites/) for your specific OS (macOS/Windows/Linux).

### Installation

1. Clone the repository:
```sh
git clone https://github.com/kanakkholwal/recast.git
cd recast
```

2. Install workspace dependencies:
```sh
pnpm install
```

### Developing

To run the desktop application in dev mode (spins up both the SvelteKit frontend and Tauri backend):
```sh
pnpm turbo run dev --filter=recast-desktop
```

To run the marketing website in dev mode:
```sh
pnpm turbo run dev --filter=recast-web
```

## 📦 Building for Production
To build the binaries for your current platform:
```sh
pnpm turbo run build --filter=recast-desktop
```

## 🖼 Wallpaper assets (external download)

The 23 full-res wallpapers (~157 MB) are **not** bundled with the installer. They
live in `assets/backgrounds/wallpapers/` and are distributed via GitHub Release
assets, then downloaded on first launch into the user's app data dir and
verified against a SHA-256 manifest. If the user is offline, tiny bundled WebP
thumbnails are shown as placeholders with an "Offline" badge; once connectivity
returns the downloader retries automatically.

**To add / update wallpapers:**

1. Drop the PNG(s) into `assets/backgrounds/wallpapers/`.
2. Run `pnpm prepare:assets-wallpapers` — this regenerates bundled WebP thumbs
   at `apps/desktop/static/backgrounds/thumbs/` and rewrites `assets/manifest.json`
   with fresh sha256 + size fields.
3. Publish a GitHub Release with the PNGs **and** the manifest:
   ```sh
   RELEASE_TAG=wallpapers-v2 pnpm prepare:assets-wallpapers
   gh release create wallpapers-v2 \
     ./assets/backgrounds/wallpapers/*.png \
     ./assets/manifest.json
   ```
4. Bump the release tag default (or set `PUBLIC_ASSETS_MANIFEST_URL` at build
   time) so the app fetches the new manifest. `PUBLIC_ASSETS_MANIFEST_URL` should
   point at the raw `manifest.json` asset URL on the release.
5. Add the wallpaper to the picker: extend `WALLPAPERS` in
   `apps/desktop/src/lib/stores/editor-store.svelte.ts`.

**Cache location** (delete to force re-download): `<appDataDir>/assets/` —
`%APPDATA%\com.nexonauts.recast\assets\` on Windows,
`~/Library/Application Support/com.nexonauts.recast/assets/` on macOS,
`~/.local/share/com.nexonauts.recast/assets/` on Linux.

## 🤝 Contributing
We welcome community contributions! Please read our [Contributing Guide](CONTRIBUTING.md) to learn about our development process, how to propose bugfixes and improvements, and how to submit pull requests.

## ⚖️ License
Recast is distributed under a **Dual-Licensing model**:
1. **Open Source (GPLv3)**: Free for personal use, educational, and open-source projects. Because the GPLv3 is a strong copyleft license, any modifications or derived works must also be open-sourced under the exact same license.
2. **Commercial License**: Required for enterprise deployment, proprietary commercial use, and closed-source redistribution. If you want to keep your modifications private or sell a derived product, you must purchase a commercial license.

See [LICENSE.md](LICENSE.md) for full legal details.
