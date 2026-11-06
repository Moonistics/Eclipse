# Built from the repository root: docker build -f crates/eclipse-telemetry-server/Dockerfile .
FROM rust:1-slim AS build
WORKDIR /src
# The root manifest only for the shared crate's workspace settings (edition, lints).
COPY Cargo.toml Cargo.toml
COPY crates/eclipse-telemetry crates/eclipse-telemetry
COPY crates/eclipse-telemetry-server crates/eclipse-telemetry-server
RUN cargo build --release --locked --manifest-path crates/eclipse-telemetry-server/Cargo.toml

FROM debian:stable-slim
COPY --from=build /src/crates/eclipse-telemetry-server/target/release/eclipse-telemetry-server /usr/local/bin/
# Railway's proxy sets X-Real-IP to the client's address; it is the header the server read first before
# ECLIPSE_TELEMETRY_CLIENT_IP_HEADER existed, so this keeps the same address for the rate limits (README "Hosting").
ENV ECLIPSE_TELEMETRY_DB=/data/telemetry.sqlite3 ECLIPSE_TELEMETRY_TRUST_PROXY=1 \
    ECLIPSE_TELEMETRY_CLIENT_IP_HEADER=x-real-ip
CMD ["eclipse-telemetry-server"]
