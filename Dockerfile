FROM rust:1.90-slim AS build
WORKDIR /app

# Pre-cache deps
COPY Cargo.toml ./
RUN mkdir src && echo "fn main() {}" > src/main.rs && \
    cargo build --release && \
    rm -rf src target/release/deps/avisafe_djilog_parser*

COPY src ./src
RUN cargo build --release

FROM debian:bookworm-slim AS runtime
WORKDIR /app
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates && \
    rm -rf /var/lib/apt/lists/*
COPY --from=build /app/target/release/avisafe-djilog-parser /usr/local/bin/avisafe-djilog-parser
ENV RUST_LOG=info
EXPOSE 8080
CMD ["/usr/local/bin/avisafe-djilog-parser"]
