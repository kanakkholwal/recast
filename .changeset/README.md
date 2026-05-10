# Changesets

Per-PR change entries for Recast. Every user-visible change should land with a
changeset so the next release's `CHANGELOG.md` and in-app "What's new" can be
assembled automatically.

## Adding a changeset

```sh
pnpm changeset
```

The interactive prompt asks which packages changed and whether the bump is
patch / minor / major. We only release `recast-desktop`, so that's the only
package you'll usually pick — `recast-web`, `@recast/design`, and `@recast/ui`
are ignored in `config.json`.

Then add a `kind:` line to the generated frontmatter so the merger knows which
section of `CHANGELOG.md` the entry belongs in. Allowed values: `added`,
`changed`, `fixed`, `deprecated`. Default if omitted: `changed`.

Example `.changeset/short-pandas-dance.md`:

```markdown
---
"recast-desktop": minor
kind: added
---

Active-preset chip in the editor toolbar with reset-to-source.
```

The body (everything after the frontmatter) is what shows up as the bullet in
`CHANGELOG.md` and in the desktop app's "What's new" panel — keep it to one
sentence, written in the imperative present tense.

## Cutting a release

```sh
# 1. Promote pending changesets into a new CHANGELOG.md section + sync
#    apps/desktop/src/constants/changelog.ts. Pass an explicit version so the
#    placeholder in source files (0.0.0-dev) doesn't drive the bump.
pnpm release:prepare 0.1.6

# 2. Review the diff, commit, push, then tag.
git add CHANGELOG.md apps/desktop/src/constants/changelog.ts .changeset
git commit -m "chore(release): 0.1.6"
git tag v0.1.6
git push origin main --tags
```

The `release-desktop.yml` workflow takes over once the tag lands — it bundles
artifacts as `recast_0.1.6_*` (sync-from-tag overrides the `0.0.0-dev`
placeholder in `tauri.conf.json`/`Cargo.toml`/`package.json`) and uses the new
`CHANGELOG.md` section as the GitHub release body via
`scripts/extract-changelog.mjs`.

## Why we don't use `changeset version` / `changeset publish`

- We don't publish to npm — Recast ships as a Tauri desktop binary, so the
  `publish` half of Changesets doesn't apply.
- Changesets' built-in changelog generator emits its own format; we already
  have a Keep-a-Changelog-shaped `CHANGELOG.md` that drives both the GitHub
  release body and the in-app "What's new". `release:prepare` preserves that
  shape and additionally regenerates the desktop's typed changelog data.
- Source-file versions live as the `0.0.0-dev` placeholder so local builds
  never look like a release. The git tag is the single source of truth at
  release time.
