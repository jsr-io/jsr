# This Dockerfile is adapted from https://whitfin.io/blog/speeding-up-rust-docker-builds/
#
# The build context is the repository ROOT (not api/), because registry_api is a
# workspace member that depends on the workspace-local `shared` and `macros`
# path crates. CI builds it with `context: .` and `file: api/Dockerfile`.

FROM rust:bookworm AS build

WORKDIR /app

# Copy the workspace + member manifests first so the dependency build caches
# independently of source changes.
COPY ./Cargo.toml ./Cargo.lock ./
COPY ./api/Cargo.toml ./api/Cargo.toml
COPY ./macros/Cargo.toml ./macros/Cargo.toml
COPY ./shared/Cargo.toml ./shared/Cargo.toml
COPY ./shared/build.rs ./shared/build.rs
COPY ./shared/license-list-data ./shared/license-list-data
COPY ./tasks/Cargo.toml ./tasks/Cargo.toml

# Stub sources so the dependency graph compiles and the dep build is cached.
RUN mkdir -p api/src macros/src shared/src tasks/src \
  && echo 'fn main() {}' > api/src/main.rs \
  && echo 'fn main() {}' > tasks/src/main.rs \
  && touch macros/src/lib.rs shared/src/lib.rs

# Enable jemalloc heap profiling support at compile time.
# prof:true enables the profiler; prof_active:true keeps sampling on so heap
# dumps reflect cumulative allocations (needed to diagnose memory growth).
ENV JEMALLOC_SYS_WITH_MALLOC_CONF=prof:true,prof_active:true

# this build step will cache your dependencies
RUN cargo build --release -p registry_api
RUN rm -rf api/src macros/src shared/src tasks/src

# copy your source tree
COPY ./api/src ./api/src
COPY ./macros/src ./macros/src
COPY ./shared/src ./shared/src
COPY ./shared/migrations ./shared/migrations
COPY ./shared/.sqlx ./shared/.sqlx

# build for release
RUN rm -f ./target/release/deps/registry_api* ./target/release/deps/libregistry_api*
RUN cargo build --release -p registry_api

# our final base
FROM debian:bookworm-20250428-slim

RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

# copy the build artifact from the build stage
COPY --from=build /app/target/release/registry_api ./registry_api

ENV RUST_BACKTRACE=1

# set the startup command to run your binary
ENTRYPOINT ["./registry_api"]
