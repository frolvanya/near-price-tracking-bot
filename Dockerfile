FROM rust:1.74 as build

WORKDIR /near-price-tracking-bot

COPY Cargo.toml Cargo.lock ./

RUN mkdir -p src && echo 'fn main() {}' > src/main.rs
RUN cargo build --release
RUN rm src/*.rs

COPY src src
RUN rm ./target/release/deps/near_price_tracking_bot*
RUN cargo build --release

FROM rust:1.74-slim

COPY --from=build /near-price-tracking-bot/target/release/near-price-tracking-bot .

ENV RUST_LOG=info
CMD ["./near-price-tracking-bot"]
