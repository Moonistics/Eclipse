# The macOS launcher: Eclipse.app in a .dmg

`package.sh BINARY VERSION OUTDIR` (run by `.github/workflows/launcher.yml` on macOS) makes:

- `Eclipse.app`: `Contents/MacOS/eclipse-launcher`, `Contents/Info.plist` (this folder's, bundle
  id `dev.eclipse.launcher`, macOS 11 or later), `Contents/Resources/eclipse.icns`
  (`packaging/icons`, from `cargo run -p xtask -- icons`).
- `eclipse-launcher-macos-arm64.dmg`: the installer. Opening it shows the app and a link to
  Applications over `background.png` / `background@2x.png` (drawn from `background.html`); the
  player drags one onto the other. Built by [dmgbuild](https://github.com/dmgbuild/dmgbuild),
  which writes the Finder layout without a GUI session.
- `eclipse-launcher-macos-arm64.zip`: the same app, zipped with `ditto`. The launcher updates
  itself from this: inside Eclipse.app it replaces the whole bundle (copied beside it, then
  swapped in by renaming, then restarted); a bare `eclipse-launcher` binary, as launchers before
  the app shipped, finds the binary inside the zip and replaces only itself.

## Signing and notarization

With `MACOS_SIGN_IDENTITY` set to a **Developer ID Application** identity, builds are signed with the
hardened runtime and a secure timestamp; with `MACOS_NOTARY_PROFILE` (a notarytool profile in
`MACOS_SIGN_KEYCHAIN`) too, they are **notarized** by Apple and **stapled**, so Gatekeeper opens them on
any Mac without a warning, offline too:

- the launcher (`package.sh`): Eclipse.app is signed, notarized and stapled first, so the zip the
  launcher updates itself from and the .dmg both hold it stapled; then the .dmg is signed,
  notarized and stapled. The script ends with what Gatekeeper says of each, which must read
  `accepted` and `source=Notarized Developer ID`.
- the game: `eclipse-client`, `eclipse-ctl` and `eclipse-prepare` are signed right after they are
  built, and the zip is notarized. Programs outside a bundle can't hold a ticket: Gatekeeper asks
  Apple online the first time each one runs.

No entitlements are needed. The launcher checks an update's signature before swapping the new
Eclipse.app in, and again where it lands; when it doesn't hold, the old app is put back. With
`MACOS_SIGN_IDENTITY` empty, builds are signed **ad hoc**.

### Gatekeeper with an ad hoc build

An ad hoc build (a fork's, or `package.sh` without `MACOS_SIGN_IDENTITY`) is consistent and runs
on Apple Silicon, but Gatekeeper doesn't know the developer. The first time it is opened after
downloading:

- macOS 14 and earlier: right-click (or Control-click) Eclipse in Applications, choose **Open**,
  then **Open** in the dialog.
- macOS 15 and later: open it once (it is refused), then **System Settings › Privacy & Security**,
  **Open Anyway** next to "Eclipse was blocked", and confirm.

After that it opens normally, and its own updates don't ask again (they aren't downloaded by a
browser, so they carry no quarantine flag).
