FROM rust

WORKDIR /usr/src
RUN USER=root cargo new --lib dummy

COPY Cargo.toml /usr/src/dummy/
COPY Cargo.lock /usr/src/dummy/
COPY rust-toolchain /usr/src/dummy/
WORKDIR /usr/src/dummy
RUN cargo build --release

WORKDIR "/project"
VOLUME [ "/project" ]
