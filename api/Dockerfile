# This Dockerfile is adapted from https://whitfin.io/blog/speeding-up-rust-docker-builds/
#
# The build context is the repository ROOT (not api/), because registry_api is a
# workspace member that depends on the workspace-local `crates/jsr_types` path
# crate. CI builds it with `context: .` and `file: api/Dockerfile`.

FROM rust:bookworm AS build

WORKDIR /app

# Copy the workspace + member manifests first so the dependency build caches
# independently of source changes.
COPY ./Cargo.toml ./Cargo.lock ./
COPY ./api/Cargo.toml ./api/Cargo.toml
COPY ./api/build.rs ./api/build.rs
COPY ./api/license-list-data ./api/license-list-data
COPY ./api/macros/Cargo.toml ./api/macros/Cargo.toml
COPY ./crates/jsr_types/Cargo.toml ./crates/jsr_types/Cargo.toml

# Stub sources so the dependency graph compiles and the dep build is cached.
RUN mkdir -p api/src api/macros/src crates/jsr_types/src \
  && echo 'fn main() {}' > api/src/main.rs \
  && touch api/macros/src/lib.rs crates/jsr_types/src/lib.rs

# Enable jemalloc heap profiling support at compile time.
# prof:true enables the profiler; prof_active:true keeps sampling on so heap
# dumps reflect cumulative allocations (needed to diagnose memory growth).
ENV JEMALLOC_SYS_WITH_MALLOC_CONF=prof:true,prof_active:true

# this build step will cache your dependencies
RUN cargo build --release -p registry_api
RUN rm -rf api/src api/macros/src crates/jsr_types/src

# copy your source tree
COPY ./api/src ./api/src
COPY ./api/migrations ./api/migrations
COPY ./api/macros ./api/macros
COPY ./api/.sqlx ./api/.sqlx
COPY ./crates/jsr_types/src ./crates/jsr_types/src

# build for release
RUN rm -f ./target/release/deps/registry_api* ./target/release/deps/libregistry_api* \
          ./target/release/deps/registry_api_macros* ./target/release/deps/libregistry_api_macros* \
          ./target/release/deps/jsr_types* ./target/release/deps/libjsr_types*
RUN cargo build --release -p registry_api

# our final base
FROM debian:bookworm-20250428-slim

RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

# copy the build artifact from the build stage
COPY --from=build /app/target/release/registry_api ./registry_api

ENV RUST_BACKTRACE=1

# set the startup command to run your binary
ENTRYPOINT ["./registry_api"]
