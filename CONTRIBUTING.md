# Contributing to Eclipse

Eclipse is Minecraft: Java Edition 26.3 rewritten in Rust. It plays exactly like vanilla, and the code stays small,
clean and readable. Pull requests are welcome, but the bar is high and most get closed if they miss it. Read this
page before you open one.

## Build it

- Rust (stable), Python 3.10+, JDK 25. On Linux also `libasound2-dev libudev-dev`.
- Eclipse never downloads the game. It prepares its resources from your own copy of Minecraft 26.3
  ([Build from source](README.md#build-from-source)).
- The launcher is its own Cargo workspace in `launcher/` (`cd launcher && cargo run`).
- Run `git config core.hooksPath .githooks` once. The hooks reject game files, decompiled source and prepared
  resources (`.local/`) before they reach a commit.

## Vanilla is the spec

Wherever vanilla has a feature, Eclipse matches it, down to the order of random calls. Parity is checked by
harnesses: a `tools/test_*_parity.py` script runs a Java oracle against the real game and a Rust probe against
Eclipse, then compares the two ([parity workflow](docs/parity-workflow.md)).

- Read the vanilla code first (`python3 tools/rea_vanilla.py method <class> <method>`) and name the classes and
  methods you followed in the pull request.
- Gameplay changes come with a harness, new or extended.
- Eclipse-only features follow [the design system](docs/design-system.md) and are off by default wherever they
  change the vanilla look.
- Everything players run is Rust. Python and Java are only for dev tools and harness drivers in `tools/`.

## What gets merged

- **One change, small.** Under about 500 lines where possible. A feature, a refactor and a formatting pass are
  three pull requests.
- **Surgical.** Touch only what the change needs. No drive-by renames, reformatting or import shuffles.
- **Fits the code around it.** Same naming, structure and error handling as the neighbouring code. Reuse the
  helper that already exists instead of writing a second one.
- **No noise.** No comments that restate the code, no debug prints, no dead code, no TODOs, no placeholder values.
- **No speculative code.** No abstraction with one user, no options nobody asked for, no handling for cases that
  can't happen.
- **Tested.** Tests check behaviour against an independent expected value or a vanilla oracle, not against the
  code under test.
- **Yours.** You understand and can explain every line. If you can't answer a review question about your own
  change, it gets closed.
- **Translation-ready.** No hardcoded player-facing strings: `tr`/`trf` in the launcher, translation keys in the
  game.

Pull requests that are mostly churn, restate existing code, or arrive with no explanation are closed without
review.

## Other projects' code

Eclipse is MIT. You may read or adapt code only from projects under MIT, MIT-0, BSD-2-Clause, BSD-3-Clause, 0BSD,
ISC, zlib, Apache-2.0, Unlicense or CC0-1.0, checked at the commit you used. Everything else (GPL, LGPL, AGPL, MPL,
EPL, Polyform, CC BY variants, source-available, unlicensed) is off limits, even to read. Copied code keeps its
notice and gets an entry in [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md). Name the project, commit and license
in the pull request.

## Before asking for review

Run the self-check and paste its output into the pull request:

```sh
python3 tools/pr_ready.py
```

It runs rustfmt, clippy with warnings denied, the tests of the crates you touched, and lists or runs the parity
harnesses your change can affect. `--quick` skips the harnesses.

## Review and merging

Only the maintainer reviews and merges. Every pull request needs that approval, merges are squashed, and `main`
never takes force pushes. Keep your branch up to date by merging `main` into it, and leave "Allow edits by
maintainers" on.

Bugs go to [issues](https://github.com/Moonistics/Eclipse/issues). For a larger change, open an issue first and wait
for a go before writing code.
