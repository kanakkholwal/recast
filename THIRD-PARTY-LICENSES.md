# Third-party licenses

Everything Recast ships, embeds, or downloads that someone else wrote, and the terms
it comes under. Organised by how it reaches the user, because that is what decides
our obligations: a binary we ship in the installer carries different duties than a
crate compiled into our own code, which differs again from a model the user chooses
to download at runtime.

Regenerate the dependency counts with `pnpm licenses list` and
`cargo metadata --all-features` (see [Regenerating](#regenerating)). Last verified
against the tree on 2026-07-21.

> This file is an attribution record, not legal advice. The items flagged
> **⚠ Needs review** are ones where our obligations depend on distribution details a
> lawyer should confirm before a public release.

---

## 1. Bundled binaries

Shipped inside the installer, executed as separate processes.

### FFmpeg / ffprobe — ⚠ Needs review

Recast invokes FFmpeg as a **sidecar executable** for every export, recording encode,
and thumbnail. We do not compile or link against it; we spawn it and talk over its
CLI and pipes.

The prebuilt binaries we ship are **GPL-licensed builds** — they bundle GPL-only
components (x264 among them):

| Platform | Source | Build |
| --- | --- | --- |
| Windows | [gyan.dev](https://www.gyan.dev/ffmpeg/builds/) | `ffmpeg-release-essentials` |
| macOS | [evermeet.cx](https://evermeet.cx/ffmpeg/) | universal2 release |
| Linux | [BtbN/FFmpeg-Builds](https://github.com/BtbN/FFmpeg-Builds) | `linux64-gpl` |

FFmpeg is © the FFmpeg developers, licensed under the
[GPL v2 or later](https://www.ffmpeg.org/legal.html) in these builds.

**Why this needs review:** shipping GPL binaries alongside a non-GPL application is
routinely done on the "separate process, arm's-length communication" basis, and our
integration fits that shape (no linking, no shared address space). But it is a
judgement call, not an automatic exemption, and it carries concrete duties either
way: the GPL text must accompany the binaries, and recipients must be able to obtain
the corresponding source for the exact build shipped. Neither is currently in the
installer. Since the standing rule is that we do not compile our own FFmpeg, the
practical fix is shipping the licence text plus a written offer pointing at the
upstream build's sources — or moving to an LGPL build if the GPL-only encoders turn
out to be unnecessary. The download scripts are in
[`scripts/release/`](scripts/release/).

---

## 2. Downloaded at runtime

Not in the installer. Fetched from Hugging Face only when the user picks a model in
**Captions → Generate**, into the app data directory.

| Model | Upstream | Licence |
| --- | --- | --- |
| Parakeet V3 / V2 (0.6B) | [nvidia/parakeet-tdt-0.6b-v3](https://huggingface.co/nvidia/parakeet-tdt-0.6b-v3), [-v2](https://huggingface.co/nvidia/parakeet-tdt-0.6b-v2) | CC-BY-4.0 |
| Nemotron Streaming 3.5 | [nvidia/nemotron-3.5-asr-streaming-0.6b](https://huggingface.co/nvidia/nemotron-3.5-asr-streaming-0.6b) | NVIDIA licence — ⚠ **Needs review** |
| Whisper Base / Small / Medium | [openai/whisper-*](https://huggingface.co/openai/whisper-medium) | Apache-2.0 |

The GGUF conversions we actually download are republished by the
[`handy-computer`](https://huggingface.co/handy-computer) Hugging Face org, the
canonical catalogue for [transcribe.cpp](https://github.com/cjpais/Handy). Model
metadata in our registry (language counts, capability flags, relative speed/accuracy
scores) is derived from
[Handy's `catalog.json`](https://github.com/cjpais/Handy/blob/main/src-tauri/src/catalog/catalog.json).
Thanks to **cj pais** and the Handy project for doing the conversion and curation work.

**CC-BY-4.0 requires attribution** — this table is it, and it needs to stay reachable
from the app, not only the repo.

**Nemotron's licence is listed upstream as `other`.** NVIDIA's model licences vary in
whether they permit commercial use and what they require downstream. Read the actual
model card before shipping this entry.

---

## 3. Ported source

Third-party code adapted into ours, rather than consumed as a dependency.

### Screenshot Studio → `@recast/application/screenshot-editor`

A Svelte port of [Screenshot Studio](https://github.com/KartikLabhshetwar/screenshot-studio)
by **Kartik Labhshetwar**, © 2025, Apache-2.0. Full text and statement of changes:
[`packages/application/NOTICE.md`](packages/application/NOTICE.md) and
[`packages/application/licenses/`](packages/application/licenses/).

---

## 4. Compiled-in dependencies

### Rust crates — 888 third-party packages

Compiled into the desktop binary. Overwhelmingly permissive; every package declares a
licence (no unlicensed crates in the tree).

| Licence | Crates |
| --- | --- |
| Apache-2.0 OR MIT | 520 |
| MIT | 221 |
| Apache-2.0 OR MIT OR Zlib | 39 |
| Unicode-3.0 | 18 |
| Apache-2.0 OR Apache-2.0-WITH-LLVM-exception OR MIT | 16 |
| Apache-2.0 | 14 |
| BSD-3-Clause | 8 |
| MIT OR Unlicense | 7 |
| MPL-2.0 | 5 |
| BSD-2-Clause | 4 |
| ISC | 4 |
| remainder (0BSD, Zlib, CC0-1.0, BSL-1.0, CDLA-Permissive-2.0, NCSA, BlueOak-1.0.0, …) | 28 |

**No GPL or AGPL crates.** The five MPL-2.0 crates (`cssparser`, `cssparser-macros`,
`dtoa-short`, `selectors`, `option-ext`) are file-level copyleft: fine to link, and we
have not modified them, so no source-disclosure duty arises.

Worth naming individually:

- **[transcribe.cpp](https://github.com/cjpais/Handy)** (`transcribe-cpp` crate) — the
  on-device speech recognition engine. Runs every GGUF model above.
- **[ocrs](https://github.com/robertknight/ocrs) / rten** — pure-Rust OCR, chosen so
  Intel Macs stay supported.
- **[Tauri](https://tauri.app)** — the desktop shell.

### npm packages — 705 third-party packages

| Licence | Packages |
| --- | --- |
| MIT | 536 |
| Apache-2.0 | 58 |
| ISC | 51 |
| BSD-3-Clause | 14 |
| MIT OR Apache-2.0 | 13 |
| BlueOak-1.0.0 | 7 |
| OFL-1.1 | 4 |
| BSD-2-Clause | 4 |
| MPL-2.0 (+1 dual MPL/Apache) | 4 |
| Unlicense | 3 |
| Unknown (declared) | 3 |
| LGPL-3.0 / Apache-2.0 AND LGPL-3.0-or-later | 2 |
| Python-2.0, 0BSD, GSAP standard | 3 |

Needing individual attention:

- **[mediabunny](https://github.com/Vanilagy/mediabunny)** (MPL-2.0) — the media
  decode engine behind the editor's preview. File-level copyleft: modifications to
  *its* files would have to be published. We consume it unmodified.
- **[@breezystack/lamejs](https://github.com/breezystack/lamejs)** (LGPL-3.0) —
  MP3 encoding in `@recast/media`. ⚠ **Needs review**: LGPL in a bundled JS build
  means the relink/replace obligation has to be satisfied somehow (unminified module,
  or a note on how to swap it).
- **[gsap](https://gsap.com/standard-license)** (GSAP Standard "no charge" licence) —
  used by `apps/web` only, not the desktop app. Free for this use, but it is a
  bespoke licence with conditions, not an OSI one.
- **Fonts** (OFL-1.1): Geist, Geist Mono, Google Sans, Inter, via
  [Fontsource](https://fontsource.org). OFL permits bundling and embedding; the fonts
  must keep their names and cannot be sold on their own.
- **[Satoshi](https://www.fontshare.com/fonts/satoshi)** (ITF Free Font License) —
  display face for `apps/web`, vendored as woff2 in `apps/web/static/fonts` because
  Fontshare faces are not published to Fontsource. The ITF FFL permits free
  commercial use and webfont embedding; the files may not be resold or redistributed
  as a font product, and the name must be preserved.
- **@polar-sh/better-auth**, **@polar-sh/sdk**, **spawndamnit** declare no licence
  field. ⚠ **Needs review** — check their repos before a release build.

Caption fonts fetched on demand for burn-in come from
[Google Fonts](https://fonts.google.com) and carry their own (typically OFL-1.1) terms.

---

## Regenerating

```bash
# npm side — grouped by licence
pnpm licenses list

# Rust side — every non-workspace package and its licence
cd apps/desktop/src-tauri && cargo metadata --format-version 1 --all-features
```

Both counts above come from those two commands. Re-run them when dependencies change
and update the tables; the **⚠ Needs review** items are the ones that matter, and they
do not change on their own.
