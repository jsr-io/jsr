# This Dockerfile is adapted from https://whitfin.io/blog/speeding-up-rust-docker-builds/
#
# The build context is the repository ROOT (not api/), because registry_api is a
# workspace member that depends on the workspace-local `shared` and `tasks`
# crates. CI builds it with `context: .` and `file: api/Dockerfile`.

FROM rust:1.99.1-bookworm@sha256:e6f2cff35d0f309acf8433b4f05878ed77092602c5065cc81801177da0bb3a9c AS build

WORKDIR /app

# Copy the workspace + member manifests first so the dependency build caches
# independently of source changes.
COPY ./Cargo.toml ./Cargo.lock ./
COPY ./api/Cargo.toml ./api/Cargo.toml
COPY ./api/build.rs ./api/build.rs
COPY ./shared/Cargo.toml ./shared/Cargo.toml
COPY ./shared/build.rs ./shared/build.rs
COPY ./shared/license-list-data ./shared/license-list-data
COPY ./tasks/Cargo.toml ./tasks/Cargo.toml

# Stub sources so the dependency graph compiles and the dep build is cached.
# shared/build.rs embeds shared/migrations; an empty directory keeps this
# layer's cache key independent of them.
RUN mkdir -p api/src shared/src shared/migrations tasks/src \
  && echo 'fn main() {}' > api/src/main.rs \
  && touch shared/src/lib.rs tasks/src/lib.rs

# Enable jemalloc heap profiling support at compile time.
# prof:true enables the profiler; prof_active:true keeps sampling on so heap
# dumps reflect cumulative allocations (needed to diagnose memory growth).
ENV JEMALLOC_SYS_WITH_MALLOC_CONF=prof:true,prof_active:true

# this build step will cache your dependencies
RUN cargo build --release -p registry_api
RUN rm -rf api/src shared/src tasks/src

# copy your source tree
COPY ./api/src ./api/src
COPY ./shared/src ./shared/src
COPY ./shared/migrations ./shared/migrations
COPY ./tasks/src ./tasks/src

# build for release
# COPY keeps the context's mtimes, so cargo would not notice the sources and
# migrations changed; the build script output has to go too.
RUN rm -rf ./target/release/deps/registry_api* ./target/release/deps/libregistry_api* \
          ./target/release/build/registry_api-* ./target/release/build/registry_api_shared-*
RUN cargo build --release -p registry_api

# our final base
FROM debian:bookworm-20250428-slim@sha256:4b50eb66f977b4062683ff434ef18ac191da862dbe966961bc11990cf5791a8d

RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

# copy the build artifact from the build stage
COPY --from=build /app/target/release/registry_api ./registry_api

ENV RUST_BACKTRACE=1

# set the startup command to run your binary
ENTRYPOINT ["./registry_api"]
