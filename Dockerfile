FROM frolvlad/alpine-rust:latest as build

RUN apk add --no-cache openssl-dev

RUN cargo new --bin near-price-tracking-bot

WORKDIR /near-price-tracking-bot

COPY Cargo.toml Cargo.lock ./

RUN cargo build --release
RUN rm src/main.rs

COPY src src
RUN rm ./target/release/deps/near_price_tracking_bot*
RUN cargo build --release

FROM --platform=linux/amd64 alpine:latest

RUN apk add --no-cache libgcc

COPY --from=build /near-price-tracking-bot/target/release/near-price-tracking-bot .

ENV RUST_LOG=info
CMD ["./near-price-tracking-bot"]

