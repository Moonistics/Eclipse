# Security

How Eclipse's downloads are checked, and how to report a problem. Releases are not set up yet
([release-flow.md](release-flow.md)); this is what the launcher and the server check once they are.

## Signed updates

Each release carries a list of its files' checksums, signed with Eclipse's release key. The launcher (for the game
and for itself) and `eclipse-server update` check that signature against the public keys built into them
(`crates/eclipse-release-sign/release-keys.txt`), then check the download against the list. Anything unsigned,
signed with another key, signed for another version, or changed in any way is refused with a message saying why.
How the files are signed and the key rotated: [release-flow.md](release-flow.md#signed-checksums).

## Nightly signatures

A Nightly carries two signatures over the same `SHA256SUMS`. `SHA256SUMS.sig` is signed for the channel
(`eclipse release nightly`). `SHA256SUMS.v2.sig` is signed for the Nightly's own version
(`eclipse release nightly-<date>-<commit>`), which its release title names (`Nightly <date> · <commit>`). The
launcher, `eclipse-server update` and `install.sh` parse that title strictly, refuse a Nightly whose title doesn't
parse, and install it only with a v2 signature for exactly that version, so an older Nightly can't be served as
tonight's. Installing an older Nightly on purpose still works as a rollback, with a warning, and never happens as an
automatic update.

## What this doesn't solve

- **Code that merges is code that ships.** Signing proves a build came from Eclipse's pipeline, not that the code
  is good. Against a bad change, the safety nets are the checks, the harnesses and review.
- **The first install** with `install.sh` checks the signature when OpenSSL 3 is installed: it verifies
  `SHA256SUMS.sig` (a Nightly's `SHA256SUMS.v2.sig`, for its own version) against the release key built into the
  script, then the archive against `SHA256SUMS`. With an older OpenSSL, LibreSSL (what macOS ships) or none, it
  says so and checks only the SHA-256 GitHub publishes. `install.ps1` checks only that SHA-256. Every update after
  that checks the signature.

## Reporting a security problem

Use GitHub's private vulnerability reporting (the repository's **Security** tab, "Report a vulnerability"), not a
public issue. Say what you found and how to reproduce it; we answer there.
