<h1 align="center">Recast</h1>

<p align="center">
  <strong>Local-first screen recording and editing.</strong> Record, polish, and share product demos from your own machine.
</p>

<p align="center">
  <a href="https://recast.nexonauts.com">Website</a>
  ·
  <a href="https://recast.nexonauts.com/features">Features</a>
  ·
  <a href="https://recast.nexonauts.com/pricing">Pricing</a>
  ·
  <a href="https://recast.nexonauts.com/download">Download</a>
  ·
  <a href="https://recast.nexonauts.com/changelog">Changelog</a>
</p>

<p align="center">
  <a href="https://github.com/kanakkholwal/recast/blob/main/LICENSE.md"><img src="https://img.shields.io/badge/license-Dual_License-blue.svg?style=flat-square" alt="License: Dual License (GPLv3 / Commercial)"></a>
  <a href="https://github.com/kanakkholwal/recast/actions/workflows/deploy-web.yml"><img src="https://github.com/kanakkholwal/recast/actions/workflows/deploy-web.yml/badge.svg" alt="Web deploy status"></a>
<a href="https://deepwiki.com/kanakkholwal/recast"><img src="https://deepwiki.com/badge.svg" alt="Ask DeepWiki"></a>
</p>

## About

Recast is an open-source screen recorder and editor that runs entirely on your
machine. No account is required to record, and nothing you record is uploaded
unless you choose to share it. Anonymous crash reporting is on by default and
product analytics is opt-in; both are tied to no account and can be switched
off in Settings. To share a finished video, push it to your own Google Drive
from the export dialog and copy the link.

A hosted sharing layer (Recast Cloud) with watch analytics and team
workspaces is on the way. It is storage-agnostic by design: bring your own
storage on the free tier, or use Recast-managed storage or a custom S3, R2,
Azure, or GCP bucket on paid plans.

For the full feature catalog, screenshots, and competitor comparisons, see
the [web site](https://recast.nexonauts.com). This README focuses on
running the codebase locally.

## Quick start

Prerequisites: Node.js 18+, [pnpm](https://pnpm.io/) 10+, Rust 1.77+, and the
[Tauri OS prerequisites](https://v2.tauri.app/start/prerequisites/) for your
platform. The setup script can auto-install any of these that are missing.

```sh
git clone https://github.com/kanakkholwal/recast.git
cd recast

# Windows (PowerShell)
powershell -ExecutionPolicy Bypass -File scripts/setup.ps1

# macOS / Linux
bash scripts/setup.sh
```

Run the desktop app in dev mode:

```sh
pnpm --filter recast-desktop dev
```

Run the website:

```sh
pnpm turbo run dev --filter=recast-web
```

Prefer to set things up by hand? See the
[manual setup steps in CONTRIBUTING.md](CONTRIBUTING.md#manual-setup).

## Contributing

The [Contributing Guide](CONTRIBUTING.md) covers the codebase mental model,
manual setup, building production binaries, the changelog and release
workflow, and how to submit pull requests.

## License

Recast is dual-licensed. The source is **GPLv3** for personal, educational,
and open-source use. A **commercial license** is required for closed-source
redistribution or proprietary derived products. See [LICENSE.md](LICENSE.md)
for the full terms.
