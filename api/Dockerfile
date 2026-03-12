# This Dockerfile is adapted from https://whitfin.io/blog/speeding-up-rust-docker-builds/

FROM rust:bookworm AS build

# create a new empty shell project
RUN USER=root cargo new --bin registry_api
WORKDIR /registry_api

# copy over your manifests
COPY ./Cargo.toml ./Cargo.toml

# this build step will cache your dependencies
RUN cargo build --release
RUN rm src/*.rs

# copy your source tree
COPY ./src ./src
COPY ./migrations ./migrations
COPY ./license-list-data ./license-list-data
COPY ./.sqlx ./.sqlx

# build for release
RUN rm ./target/release/deps/registry_api*
RUN cargo build --release

# our final base
FROM debian:bookworm-20250428-slim

RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

# copy the build artifact from the build stage
COPY --from=build /registry_api/target/release/registry_api ./registry_api
COPY --from=build /registry_api/license-list-data ./license-list-data

ENV RUST_BACKTRACE=1
ENV LICENSE_LIST_DATA_PATH=/license-list-data

# set the startup command to run your binary
ENTRYPOINT ["./registry_api"]
