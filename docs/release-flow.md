# Release flow

How a change gets from a pull request to players: the checks a pull request passes and, once they exist,
releases.

## Pull request checks

The `checks` workflow (`.github/workflows/ci.yml`) runs on GitHub's `ubuntu-latest` runners for every pull request
and every push to main, in three jobs:

| Job | What |
| --- | --- |
| `game` | rustfmt, clippy with warnings denied (the workspace, and the server build: `eclipse-client` without default features), `cargo test --workspace --lib --bins --tests` |
| `launcher` | The launcher's own workspace (`launcher/`): rustfmt, clippy with warnings denied, `cargo test --locked --lib --bins` |
| `python` | The public-tree check (`tools/check_public_tree.py`: a pull request's own commits, main's whole history), then `python -m pytest -q tests` |

The vanilla harnesses need your own copy of Minecraft, which CI doesn't have, so it doesn't run them. Run the ones
your change affects before asking for review: `python3 tools/pr_ready.py` picks them
([parity-workflow.md](parity-workflow.md)).

## Releases

Releases are not set up yet: no workflow builds or publishes them. The launcher (game installs, updates and its
own updates), `eclipse-server update` and `install.sh` read the releases of `Moonistics/Eclipse`; until there is
one they find none and say so. `tools/release_train.py` (when a release goes out, its tag and its notes) and the
tag scheme in [versioning.md](versioning.md) stay for when they are.

### Signed checksums

Every release is to carry `SHA256SUMS` (the archives' SHA-256, `sha256sum`'s format) and `SHA256SUMS.sig`: one
base64 Ed25519 signature per line over `eclipse release <tag>\n` followed by SHA256SUMS, so a signed list can't be
reused under another tag. The launcher and `eclipse-server update` check it against the public keys built in from
`crates/eclipse-release-sign/release-keys.txt`, then each download against the list.

- `tools/ci/sign_release.sh` makes both files with `sha256sum` and OpenSSL and nothing else, so the job that holds
  the private key runs no code a pull request can change.
- `tools/ci/new_release_key.sh` pipes a new key from `eclipse-release-sign keygen` straight into
  `gh secret set RELEASE_SIGNING_KEY --env release` and prints only the public key, which then goes into
  release-keys.txt by pull request.

**Rotating the key:** make the next key under the other name (`RELEASE_SIGNING_KEY_NEXT`), add its public key to
release-keys.txt beside the old one, sign with both until launchers and servers built with both are in use, then
delete the old key's secret and its line. If a key may have leaked, remove its line at once: launchers built
before the new key then refuse new versions until players download the launcher again.
