# Opt-in data collection

Eclipse collects nothing unless the player says yes, and sends nothing unless `ECLIPSE_TELEMETRY_URL` names a server: there is none by default, and without one the launcher never asks. With one, the launcher asks once, with four separate switches that are all off until the player turns one on, and the same switches stay in Settings. The game and the launcher obey the same answer. The code is in `crates/eclipse-telemetry`.

Not part of it: the cosmetics service's *Show me as Eclipse* (Extras › Cosmetics, only when cosmetics run, which they don't by default): a signed-in account tells the cosmetics service, at start and then every hour, that it plays Eclipse, so other Eclipse players see the Eclipse mark beside its name. The service reports it for 3 hours, and anyone can ask it whether an account played Eclipse in that window. Turning the setting off makes the service forget the account at once. It is not part of telemetry and the switches above don't cover it: see [cosmetics.md](cosmetics.md).

## Consent

`telemetry/consent.json` in the game folder (`ECLIPSE_HOME`, else `eclipse` in the platform data folder):

```json
{ "version": 2, "answered": 1792060800, "choices": { "performance": true, "crashes": true, "bugs": false, "benchmarks": false }, "install_id": "3f2c…" }
```

- `version` is the consent version the player answered (`CONSENT_VERSION`). It goes up when what is collected changes. The launcher then asks again, and until the player answers nothing new is kept or sent.
- `answered` is when they answered, in seconds since 1970 (UTC).
- `install_id` is 128 random bits, made the first time the player says yes to anything. It ties their items together so they can be deleted, and nothing else. "Delete my data" asks the server to delete everything under it, then replaces it.

Nothing leaves the machine without consent:

- `outbox::push` keeps an item only if its kind is allowed.
- `upload::flush` reads the consent again before sending and deletes any waiting item whose kind the player has turned off since, so with everything off it never opens a connection.

`tests/consent_gating.rs` checks this against a local mock server that counts connections.

## What is sent

Every item has `day` (days since 1970, UTC: no finer time), `source` (`launcher` or `game`), `version` (the release), `kind` and `data`.

| Kind | Switch | `data` | Largest | A day |
|---|---|---|---|---|
| `perf` | performance | `system`, `settings` (render and simulation distance, max FPS, vsync, fullscreen, graphics mode), `seconds` played, `frames` and `ticks` summaries, `peak_ram_mb`, `to_title_ms`, `to_world_ms`, `slow_frames` | 4 KiB | 12 |
| `crash` | crash reports | `system`, `error`, `stack` (one frame a line), `thread`, the graphics and settings details, all scrubbed. No log, no world, no server | 32 KiB | 5 |
| `bug` | bug reports | `system`, the player's `text` (up to 4000 characters), and the `log` only if they add it after reading it | 80 KiB | 5 |
| `benchmark` | benchmark uploads | the benchmark report as `docs/benchmark-report.md` defines it, as it is, with its own schema field | 128 KiB | 10 |

`system` describes the machine, with what can explain its performance. A field the OS doesn't give is empty or 0:

| Field | What | Example |
|---|---|---|
| `os`, `os_version`, `arch` | the OS, its version, the CPU architecture | `linux`, `Fedora Linux 43`, `x86_64` |
| `cpu`, `threads`, `cores` | the CPU's brand name (on Windows the registry's; its family and model, `PROCESSOR_IDENTIFIER`, only where the registry has none), its threads and physical cores | `13th Gen Intel(R) Core(TM) i7-13700KF`, 24, 16 |
| `cpu_base_mhz`, `cpu_boost_mhz` | base and boost clock, where the OS gives them (not on Apple silicon) | 3400, 5400 |
| `ram_gb` | installed memory, rounded to the GiB | 32 |
| `gpu`, `gpu_vendor`, `gpu_kind` | the GPU the game draws with, its maker, `integrated` or `discrete` | `NVIDIA GeForce RTX 4080`, `nvidia`, `discrete` |
| `vram_mb` | a discrete GPU's own memory, rounded up to 256 MiB | 16384 |
| `driver`, `backend` | the driver version and the graphics API (`Vulkan`, `Metal`, `Dx12`) | `580.95.05`, `Vulkan` |
| `storage` | the drive under the game folder: `nvme`, `ssd` or `hdd` | `nvme` |
| `display` | the screen the window opened on: `width`, `height` (physical pixels), `refresh_hz`, `scale` | 2560, 1440, 165, 1.0 |
| `power`, `form` | on mains or battery (`ac`, `battery`), `laptop` or `desktop` | `ac`, `desktop` |

The launcher's own items (bug reports sent from it) have no GPU or screen: only the game knows them. The memory given to the game is not a setting in Eclipse (no Java heap): `peak_ram_mb` is what it used. Benchmark reports carry the same facts in their `hardware` (`docs/benchmark-report.md`).

No serial number, host name, user name, MAC address or path is read. Settings → Data collection → "View what would be sent" and `eclipse-launcher data show` print every field as it would leave this machine.

A summary (`frames`, `ticks`) is the count, the 50th, 95th and 99th percentile and the maximum in milliseconds (to the tenth), and 14 bucket counts: under 4 ms, 4–6, 6–8, 8–10, 10–12, 12–14, 14–16.7, 16.7–20, 20–25, 25–33.3, 33.3–50, 50–100, 100–250, and 250 ms and more. Raw frame times never leave: `Timings` records them in tenths of a millisecond (20 KB however long the session) and only its summary is kept. A perf item is under 1 KiB.

`slow_frames` says what froze: the frames of 250 ms or more while playing, counted by the step of the frame they spent most of their time in. A step is a span of the render thread (`spans.rs`), named in the code and the same in every game of a release: the slowest outer span, then the span inside it that took at least half of it, as a path (`renderer/r_uploads`, `tick/tick_mobs`; empty when no step was timed). Each entry has the step, its frame count, the longest of those frames and the longest the step took. At most 16 entries; later steps count under `other`. With performance statistics on, the render thread times its steps (wall time only, two clock reads a span); with them off it times nothing. Most sessions have no slow frame and send no `slow_frames`.

Never sent: account names or UUIDs, world names, server or LAN addresses, chat, file paths with the user's name, IP addresses. `scrub::Scrubber` replaces the user folder in paths (`/Users/<user>/…`, `C:\Users\<user>\…`), the user name, e-mail and IP addresses and UUIDs. It also replaces the terms the caller passes (the world's name, the server's address, account names). The server does not store the sender's IP address.

## Storage and sending

- `telemetry/outbox/` holds one JSON file per item, `<day>-<kind>-<random>.json`. It never holds more than 200 items or 2 MiB, and the oldest go first. `kept.json` counts today's items per kind against the caps above.
- The launcher sends the outbox when it starts and when it closes. Items go as one JSON batch (`{"schema":1,"install_id":…,"items":[…]}`, up to 64 items and 1 MiB), compressed with zstd: `POST /v1/batch` with `Content-Encoding: zstd`. A day's perf summaries are under 2 KB on the wire.
- After a failure the launcher waits a minute, doubling up to a day, and sends at most 8 batches a day. A batch the server refuses with a 4xx (not 429) is dropped.
- There is no server by default: `ECLIPSE_TELEMETRY_URL` names one (`crates/eclipse-telemetry-server`); unset, empty or `off`, nothing is sent.
- What is sent still depends on the switches: with every switch off nothing leaves, and the outbox stays empty.
- `SCHEMA` versions the format. New optional fields keep it. A field that changes meaning or goes bumps it, and the server keeps reading the older ones.

## The live count

The game sends no "I'm playing" ping, so the launcher has no live count to show. The server still takes one (`POST /v1/live`, `eclipse_telemetry::live`: a random session ID, the version, the platform and whether a LAN world is open, nothing else) and serves the count and its history.

## Server

- `POST /v1/batch`
- `DELETE /v1/installs/<install_id>`: deletes everything sent under that ID.
- `GET /v1/aggregate`: the anonymous totals the website's results page reads. Its `breakdowns` split the play sessions of the period by hardware class (below), each class with its installs, sessions, hours and mean p50 and p99 frame times. Its `slow_frames` list what froze, per release and step: the slow frames, the sessions and installs that had one, and the longest frame and step, for steps at least as many installs had as the groups need.
- `GET /v1/benchmarks/summary`: players' benchmark uploads grouped by GPU, CPU, render distance and game for the website's results. A group shows only with at least 3 different installs, and only its medians and counts: no install ID, day, OS version, driver, exact memory or screen. Uploads are never published one by one. Each group's `breakdowns` count its installs per hardware class.
- `GET /v1/export/benchmarks` and `GET /v1/export/hardware`: for the maintainers' performance work only, behind a bearer token the public cannot use (404 while the server has none). The raw benchmark reports and the play sessions per machine description, including CPU threads and physical cores, with no minimum group size. Nothing identifying: no install ID, and only what a player already agreed to send (this page's lists).
- `POST /v1/live` and `GET /v1/live`: the "I'm playing" ping and the live count.
- `GET /v1/live/history?range=24h|7d|30d|all&series=players|official`: the live count, or the official server's, over time (above).

Public output never holds a raw hardware value, only these classes (`src/classes.rs`), and a class is published only with at least 3 different installs:

| Breakdown | Classes |
|---|---|
| `ram` | 4 GB or less, 8 GB, 16 GB, 32 GB, 64 GB or more |
| `vram` | shared (integrated GPU), 2 GB or less, 4, 6, 8, 12, 16 GB, more than 16 GB |
| `storage` | nvme, ssd, hdd |
| `os` | linux, macos, windows |
| `gpu_kind` | integrated, discrete |

The driver, OS version, clocks, screen and power are kept raw for the 180 days of retention, to explain a slow group, and never published.

`crates/eclipse-telemetry-server` is the server (one SQLite file). Its [README](../crates/eclipse-telemetry-server/README.md) covers storage (about 190 bytes per play session), retention (180 days of raw rows, anonymous daily totals kept), rate limits and hosting.
