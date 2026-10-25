#!/usr/bin/env bash
# Compila in un container con limite di memoria (DESIGN §1: la VM ha poca RAM e lo swap pieno).
# Uso: ./build.sh [check|test|release|release-fast|image]
set -euo pipefail
cd "$(dirname "$0")"
mode=${1:-release}
img=rust:1.99.1-slim-trixie
run() {
  docker run --rm -m 1600m --memory-swap 1600m --user "$(id -u):$(id -g)" \
    -e CARGO_HOME=/work/.cargo-home -e CARGO_BUILD_JOBS=2 -e CARGO_INCREMENTAL=0 -e CARGO_TERM_COLOR=never \
    -v "$PWD":/work -w /work "$img" "$@"
}
case "$mode" in
  check) run cargo check --locked --all-targets ;;
  test) run cargo test --locked ;;
  release|release-fast) run cargo build --locked --profile "$mode" --bin pumpbot ;;
  image)
    run cargo build --locked --profile release --bin pumpbot
    docker build -t kraken/pumpbot:latest .
    ;;
  *) echo "modo sconosciuto: $mode" >&2; exit 2 ;;
esac
