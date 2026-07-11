# winget (Windows Package Manager) setup

The `publish-winget` job in `release-desktop.yml` submits a manifest update to
[microsoft/winget-pkgs](https://github.com/microsoft/winget-pkgs) on every
release, so Windows users get new versions with `winget upgrade Recast`. It
uses the NSIS installer asset (`recast_<version>_x64-setup.exe`).

The job no-ops until `WINGET_TOKEN` is set, so releases stay green before this
is configured. Below is the one-time bootstrap.

## Package identifier

`Nexonauts.Recast` (publisher `Nexonauts`, package `Recast`), matching the
`publisher` in `tauri.conf.json`. To change it, edit the `identifier` in the
`publish-winget` job.

## One-time bootstrap

1. **Fork winget-pkgs.** Fork `microsoft/winget-pkgs` into the account whose
   token you will use. `winget-releaser` pushes the manifest branch to this
   fork, then opens a PR upstream.

2. **Create the token.** A classic PAT with `public_repo` scope (or a
   fine-grained token with Contents + Pull requests write on the fork). Add it
   to the recast repo as the `WINGET_TOKEN` secret. Do not use the default
   `GITHUB_TOKEN`; it cannot push to your fork.

3. **Submit the first version manually.** `winget-releaser` updates *existing*
   packages; the first version of a new package is added by hand and reviewed
   by winget moderators:

   ```powershell
   winget install wingetcreate
   wingetcreate new https://github.com/kanakkholwal/recast/releases/download/vX.Y.Z/recast_X.Y.Z_x64-setup.exe
   ```

   Set `PackageIdentifier` to `Nexonauts.Recast`, installer type `nullsoft`,
   silent switch `/S`. `wingetcreate submit --token <PAT>` opens the PR. Once it
   merges, the package exists in winget-pkgs.

4. **Automated after that.** Every subsequent tagged release runs
   `publish-winget`, which computes the installer hash from the release asset
   and opens an update PR to winget-pkgs. No manual steps.

## Notes

- **Silent install.** NSIS installs silently with `/S`, which winget requires;
  no extra config needed.
- **Signing.** Unsigned installers pass winget validation but still show
  SmartScreen on launch. Once SignPath signing is live, the installer is signed
  and the winget listing reads as trusted.
- **Validation.** winget-pkgs runs automated manifest + sandbox-install checks
  and a moderator review. A failed check comments on the PR with the reason.
