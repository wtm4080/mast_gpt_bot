FROM rust:1.82 as builder

WORKDIR /app

# 依存だけ先にコピーしてビルドキャッシュ効かせる
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release || true

# 本体
COPY src ./src
RUN cargo build --release

# ランタイム用の軽いイメージ
FROM debian:stable-slim

RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /app/target/release/mastodon_gpt_bot /usr/local/bin/mastodon_gpt_bot

# 環境変数は docker-compose 側で渡す
ENTRYPOINT ["/usr/local/bin/mastodon_gpt_bot"]
