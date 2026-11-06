# Eclipse

Minecraft: Java Edition 26.3, rewritten in Rust: the game client, the dedicated server and a launcher. Eclipse is a fork of Horizon, checked against the Java game by differential harnesses.

> Not an official Minecraft product, nor approved by or associated with Mojang or Microsoft. Eclipse contains no Minecraft code, assets or data: you need your own copy of Java Edition 26.3.

There are no releases yet. To try it, build it from source.

## Build from source

Needs Rust 1.90+, Python 3.10+, JDK 25, and on Linux `libasound2-dev libudev-dev`. `eclipse-prepare` builds the game's resources from **your own copy** of Java Edition 26.3 (checked against `research-lock.json`) and never downloads it:

1. **Client:** play 26.3 once in the official launcher. Eclipse reads `~/.minecraft` (or `--minecraft-dir` / `ECLIPSE_MINECRAFT_DIR`).
2. **Server jar:** the 26.3 server jar (`downloads.server.url` in `versions/26.3/26.3.json`), passed once with `--server-jar` or `ECLIPSE_SERVER_JAR`.

```sh
git clone https://github.com/Moonistics/Eclipse.git
cd Eclipse
cargo run --release -p eclipse-prepare -- --prepare --server-jar ~/Downloads/server.jar   # needs JDK 25
cargo run --release -p eclipse-prepare -- sounds   # optional: sounds from your launcher install
cargo run --release -p eclipse-client
```

The launcher is its own workspace: `cargo run --release` in `launcher/` (on Linux it also needs `libxkbcommon-x11-dev libwayland-dev libx11-xcb-dev libfontconfig-dev libvulkan-dev`).

Prepared files land in the gitignored `.local/`: never commit or share them. Options, controls and architecture: [the native client guide](docs/native-client.md); all developer docs: [docs/README.md](docs/README.md).

## Contributing

Read [CONTRIBUTING.md](CONTRIBUTING.md). Before a pull request, run the self-check and paste its output:

```sh
git config core.hooksPath .githooks
python3 tools/pr_ready.py
```

## Licence

[MIT](LICENSE), for the project's original work; the licence keeps the copyright of Horizon's authors, whose code Eclipse started from. Dependencies keep their own licences ([notices](THIRD_PARTY_NOTICES.md)). Minecraft is a trademark of Mojang Synergies AB; its code and assets belong to Mojang and Microsoft under the [Minecraft EULA](https://www.minecraft.net/en-us/eula).
