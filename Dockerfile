FROM rust:alpine3.14 as builder

WORKDIR /app

RUN apk add musl-dev

# create a new empty project
RUN cargo init

COPY ./src src
COPY ./vendor vendor
COPY Cargo.toml Cargo.lock ./

# build dependencies, when my source code changes, this build can be cached, we don't need to compile dependency again.
RUN cargo build

# remove the dummy build.
RUN cargo clean -p tjcounter-rust

# build with x86_64-unknown-linux-musl to make it run with alpine.
RUN cargo install --path . --target=x86_64-unknown-linux-musl

# second stage.
FROM alpine:3.14
COPY --from=builder /usr/local/cargo/bin/* /usr/local/bin

EXPOSE 8182

CMD ["/usr/local/bin/tjcounter-rust"]