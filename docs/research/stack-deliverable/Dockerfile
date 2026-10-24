# syntax=docker/dockerfile:1.7
# Build:  docker build -t kraken/pumpbot:dev .
# Con gRPC: docker build --build-arg FEATURES=grpc -t kraken/pumpbot:grpc .
# Iterazione veloce: docker build --build-arg PROFILE=release-fast -t kraken/pumpbot:fast .
ARG CHEF_IMAGE=lukemathwalker/cargo-chef:0.1.78-rust-1.99.1-slim-trixie
ARG RUNTIME_IMAGE=gcr.io/distroless/cc-debian13:nonroot

FROM ${CHEF_IMAGE} AS chef
WORKDIR /app
# CARGO_BUILD_JOBS=3: la VM ha ~2,1 GB liberi e swap pieno. 4 = piu' veloce ma ~+0,3 GB di picco.
ENV CARGO_BUILD_JOBS=3 \
    CARGO_INCREMENTAL=0 \
    CARGO_NET_RETRY=5 \
    CARGO_TERM_COLOR=never

FROM chef AS planner
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
ARG FEATURES=""
ARG PROFILE=release
# .cargo/config.toml PRIMA del cook: rustflags identici tra cook e build, altrimenti le deps si ricompilano
COPY .cargo ./.cargo
COPY --from=planner /app/recipe.json recipe.json
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/usr/local/cargo/git,sharing=locked \
    cargo chef cook --profile "${PROFILE}" --locked --recipe-path recipe.json ${FEATURES:+--features $FEATURES}
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/usr/local/cargo/git,sharing=locked \
    cargo build --profile "${PROFILE}" --locked --bin pumpbot ${FEATURES:+--features $FEATURES} \
 && cp "target/${PROFILE}/pumpbot" /pumpbot

# Runtime: glibc trixie (stessa base del builder) + ca-certificates + tzdata, niente shell.
FROM ${RUNTIME_IMAGE}
COPY --from=builder /pumpbot /usr/local/bin/pumpbot
WORKDIR /data
ENTRYPOINT ["/usr/local/bin/pumpbot"]
CMD ["--config", "/config/pumpbot.toml"]
