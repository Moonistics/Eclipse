# The Play with friends relay, for anyone to run (docs/play-with-friends.md, "Running your own relay").
# Build context: the repository root (the workspace's manifests and lockfile):
#   docker build -f crates/eclipse-relay/Dockerfile -t eclipse-relay .
FROM rust:1-slim-bookworm AS build
WORKDIR /src
COPY . .
RUN cargo build --release --locked -p eclipse-relay \
    && cp target/release/eclipse-relay /eclipse-relay

FROM debian:bookworm-slim
COPY --from=build /eclipse-relay /usr/local/bin/eclipse-relay
ENV PORT=7000
EXPOSE 7000
USER nobody
CMD ["eclipse-relay"]
