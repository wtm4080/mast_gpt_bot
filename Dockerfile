# ===== ビルド用ステージ =====
FROM rust:1.82-bullseye AS builder

WORKDIR /usr/src/app

# 依存関係だけ先に解決してキャッシュを効かせる
COPY Cargo.toml Cargo.lock ./
# 必要ならここで dummy src を置いて先に依存だけビルドする手もあるけど、
# シンプルに全部コピーでもOK
COPY src ./src

# リリースビルド
RUN cargo build --release

# ===== 実行用ステージ =====
FROM debian:bookworm-slim

# TLS用のルート証明書だけ入れて軽量に
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates \
 && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# ビルドしたバイナリをコピー
# バイナリ名は crate 名と同じ `mast_gpt_bot` を想定
COPY --from=builder /usr/src/app/target/release/mast_gpt_bot /app/mast_gpt_bot

# prompts.json を含む config ディレクトリをコピー
COPY config ./config

# ログ欲しければ適当に
ENV RUST_LOG=info

# コンテナ起動時に実行するコマンド
CMD ["./mast_gpt_bot"]
