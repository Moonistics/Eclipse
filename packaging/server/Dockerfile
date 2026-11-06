# eclipse-server in a container (docs/server.md, section 3). Build context: the repository root:
#   docker build -f packaging/server/Dockerfile -t eclipse-server .
#
# The image holds none of Minecraft's files. Prepare them on the host from your own Minecraft 26.3
# (eclipse-prepare --prepare, see packaging/PLAY.txt) and mount the folder it writes, read-only:
#   docker run -d --name eclipse -p 25565:25565 --stop-timeout 180 \
#     -v eclipse-data:/data \
#     -v /path/to/.local/eclipse-client/26.3:/opt/eclipse/.local/eclipse-client/26.3:ro \
#     -e ECLIPSE_EULA=true -e ECLIPSE_WHITELIST=true -e ECLIPSE_PLAYERS=Alex,Steve -e ECLIPSE_OP=Alex \
#     eclipse-server
#
# ECLIPSE_EULA=true accepts the Minecraft EULA (https://aka.ms/MinecraftEULA) for this server. The first
# start writes /data/server.toml from ECLIPSE_* (EULA, WORLD, SEED, PORT, WHITELIST, PLAYERS, OP); later
# starts use the file. Manage it with `docker exec eclipse eclipse-server status|cmd|backup|logs ...`.
FROM rust:1-slim-bookworm AS build
WORKDIR /src
COPY . .
# The server build of the game (no sound, no controllers: it needs neither ALSA nor udev) as eclipse-engine.
RUN cargo build --release --locked -p eclipse-client --no-default-features --bin eclipse-client \
    && cargo build --release --locked -p eclipse-server --bin eclipse-server \
    && mkdir /out && cp target/release/eclipse-client /out/eclipse-engine && cp target/release/eclipse-server /out/

FROM debian:bookworm-slim
RUN useradd --system --home-dir /data --shell /usr/sbin/nologin eclipse \
    && mkdir -p /data /opt/eclipse/.local/eclipse-client \
    && chown eclipse:eclipse /data
COPY --from=build /out/ /opt/eclipse/
ENV PATH=/opt/eclipse:$PATH \
    ECLIPSE_NO_AUDIO=1
WORKDIR /data
VOLUME /data
EXPOSE 25565
USER eclipse
# SIGTERM makes eclipse-server stop the game cleanly (it saves first): give it --stop-timeout 180.
CMD ["sh", "-c", "test -f server.toml || eclipse-server init --yes || exit 1; exec eclipse-server run"]
