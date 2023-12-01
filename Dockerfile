FROM rust:1.74 as build

RUN USER=root cargo new --bin near-price-tracking-bot 
WORKDIR /near-price-tracking-bot

COPY Cargo.toml Cargo.lock ./

RUN cargo build --release && rm src/*.rs

COPY src src

RUN rm ./target/release/deps/near_price_tracking_bot*
RUN cargo build --release

FROM rust:1.74

COPY --from=build /near-price-tracking-bot/target/release/near-price-tracking-bot .

ENV RUST_LOG=info
CMD ["./near-price-tracking-bot"]
