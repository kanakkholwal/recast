# Homebrew Cask tap setup

The `publish-homebrew-cask` job in `release-desktop.yml` runs
[`update-homebrew-cask.sh`](update-homebrew-cask.sh) on every release: it
downloads the published DMGs, computes their SHA256s, renders
[`recast.rb.template`](recast.rb.template), and pushes the result to the
`kanakkholwal/homebrew-recast` tap as `Casks/recast.rb`. Mac users then install
with a quarantine-free path (no Gatekeeper "is damaged" error).

The job no-ops until `HOMEBREW_TAP_TOKEN` is set, so releases stay green before
this is configured. Below is the one-time bootstrap.

## One-time bootstrap

1. **Create the tap repo.** A Homebrew tap must be a public repo named
   `homebrew-<tap>`. Create `kanakkholwal/homebrew-recast` (empty is fine; the
   release job writes `Casks/recast.rb` on the first release). To use a
   different location, set the `TAP_REPO` env var on the job.

2. **Create the token.** A classic PAT with `repo` scope on the tap repo (or a
   fine-grained token with Contents write on `homebrew-recast`). Add it to the
   recast repo as the `HOMEBREW_TAP_TOKEN` secret. The default `GITHUB_TOKEN`
   cannot push to a different repo, so a PAT is required.

3. **Cut a release.** On the next tagged release, the job downloads the
   `recast_<version>_aarch64.dmg` and `recast_<version>_x64.dmg` assets, hashes
   them, and pushes the rendered cask. No manual formula editing.

## User install

```sh
brew install --cask kanakkholwal/recast/recast
# or, after a one-time `brew tap kanakkholwal/recast`:
brew install --cask recast
```

## Notes

- **DMGs must be on the release.** The macOS build legs produce them; the script
  hashes the live release assets (not CI artifacts) so the formula's hashes
  match exactly what users download.
- **Updates.** `auto_updates true` in the cask defers to Tauri's in-app updater,
  so `brew upgrade` does not fight it. `brew livecheck` still detects new tags.
- **Uninstall.** `brew uninstall --cask --zap recast` also removes the
  `com.kanakkholwal.recast` user-data directories listed in the cask's `zap`.
- **Identifier drift.** The cask's `zap` paths mirror the macOS
  `CFBundleIdentifier`. If `tauri.conf.json#identifier` ever changes, update the
  template's `zap` list to match.
