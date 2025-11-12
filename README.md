# mast_gpt_bot

Mastodon のメンションに反応して返信＆フリートゥートを投げる、Rust 製の GPT ボット。
OpenAI Responses API を使い、必要なら Web 検索で最新情報も拾える。  
Docker / Compose でも動かせるし、ローカルの `cargo run` でもOK。

---

## ✨ Features

- Mastodon でメンションを受けると自動返信
- 定期の「自由トゥート（free toot）」生成
- OpenAI Responses API ベースの会話生成
- **Web 検索ツール**で最新情報を参照
- プロンプトを `config/prompts.json` で管理
- Docker / Docker Compose 対応
- `cargo fmt` / `clippy` による整形・静的解析

---

## 📦 Repo structure

```
mast_gpt_bot/
├─ Cargo.toml
├─ Cargo.lock
├─ .env.example
├─ Dockerfile
├─ docker-compose.yml
├─ config/
│  ├─ prompts.json           # システム/ユーザー向けテンプレ群（Vec<ChatMessage>）
│  └─ ...                    # 必要に応じて増える
└─ src/
   ├─ main.rs
   ├─ config.rs              # BotConfig（ENVローディング）
   ├─ mastodon_api/
   │  └─ post.rs             # 投稿ユーティリティ（configを1回だけ渡す）
   ├─ notification_stream/
   │  └─ mod.rs              # Streaming API リスナー
   └─ openai_api/
      ├─ mod.rs
      ├─ stream.rs           # Responses API 呼び出し
      ├─ types.rs            # リクエスト/レスポンス型 & tools（web_search_preview）
      ├─ free_toot.rs        # 自由トゥート生成（JST時刻をsystemで注入）
      └─ reply.rs            # メンション返信生成（JST時刻をsystemで注入）
```

---

## 🔧 Requirements

- Rust stable（1.75+ 推奨）
- `rustup`（`rustfmt`, `clippy` コンポーネント）
- Docker / Docker Compose（任意）
- Mastodon アカウント & アプリトークン
- OpenAI API Key（Responses API）

---

## 🚀 Setup

### 1) Clone

```bash
git clone https://github.com/wtm4080/mast_gpt_bot.git
cd mast_gpt_bot
```

### 2) Create `.env`

`.env.example` をコピーして `.env` を作る：

```bash
cp .env.example .env
```

主な項目：

```
# OpenAI
OPENAI_API_KEY=sk-xxxx
OPENAI_MODEL=gpt-4o-mini

# Mastodon
MASTODON_BASE_URL=https://your.instance.example
MASTODON_ACCESS_TOKEN=xxxxxx

# Streaming（通常は /api/v1/streaming でOK）
# MASTODON_STREAMING_URL=wss://your.instance.example/api/v1/streaming

# Bot behavior
REPLY_MIN_INTERVAL_MS=3000
FREE_TOOT_INTERVAL_MIN=60

# Tools (optional)
ENABLE_WEB_SEARCH=true
```

> Streaming URL はインスタンスによって `/api/v1/streaming` が必要なことがある。エラー時はここを要チェック。

---

## ▶️ Run (Local)

```bash
cargo run
```

---

## 🐳 Run (Docker)

### Build

```bash
docker build -t mast-gpt-bot:latest .
```

### Run (single container)

```bash
docker run --rm   --env-file .env   mast-gpt-bot:latest
```

### Run (Compose)

```bash
docker compose up --build
```

---

## 🧠 Prompts

`config/prompts.json` に、以下のセクションがある想定：

- `free_toot_morning` / `free_toot_day` / `free_toot_night` … 自由トゥート用テンプレ（Vec<ChatMessage>）
- `reply_with_context` / `reply_without_context` … 返信テンプレ（Vec<ChatMessage>）

アプリ側はこの **テンプレ Vec<ChatMessage>** を読み、必要に応じて **JST 現在時刻** を `system` メッセージとして追記する。

---

## 🔍 Web Search (Preview)

- OpenAI Responses API の **Hosted Tool (web_search_preview)** を有効にすると、モデルが必要判断で検索→引用付き回答できる
- ON/OFF は `.env` の `ENABLE_WEB_SEARCH=true/false` で切替（コード側で tools を渡す）

---

## 🧹 Formatting & Lint

```bash
# 全クレート整形
cargo fmt --all

# チェックのみ（失敗時に非0）
cargo fmt --all -- --check

# Lint
cargo clippy -- -D warnings
```

ルートに `rustfmt.toml` を置けば、全体でスタイル共有ができる。

---

## 🧪 Troubleshooting

**Streaming が受け取れない**
- `MASTODON_STREAMING_URL` を `wss://<host>/api/v1/streaming` に修正（インスタンス依存）

**返答が古い**
- `.env` の `ENABLE_WEB_SEARCH=true` にして、Web 検索ツールを利用するようにする

---

## 📜 License

This project is licensed under the **MIT License**.  
See [LICENSE](./LICENSE) for details.
