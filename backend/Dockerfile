# Built from the repository root: docker build -f backend/Dockerfile .
FROM rust:1-bookworm AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock rustfmt.toml ./
COPY backend backend
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --release -p anycardz-api && cp target/release/anycardz-api /anycardz-api

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends libpq5 ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10001 anycardz && mkdir /cache && chown anycardz /cache
COPY --from=build /anycardz-api /usr/local/bin/anycardz-api
USER anycardz
ENV ANYCARDZ_BIND=0.0.0.0:8080 ANYCARDZ_DATA=/data ANYCARDZ_CACHE=/cache RUST_LOG=info
EXPOSE 8080
CMD ["anycardz-api"]
