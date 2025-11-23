# mast_gpt_bot

Mastodon のメンションに反応して返信＆フリートゥートを投げる、Rust 製の GPT ボット。
OpenAI Responses API（Responses v2）を使い、必要なら Web 検索で最新情報も拾える。
Docker / Compose でも動かせるし、ローカルの `cargo run` でもOK。

---

## ✨ Features

- Mastodon でメンションを受けると自動返信（会話スレッド単位で OpenAI の previous_response_id を継続保存）
- 定期の「自由トゥート（free toot）」生成
- OpenAI Responses API ベースの会話生成（返信用と FT 用でモデルを分離可）
- **Web 検索ツール**で最新情報を参照（強制検索のキーワード判定もあり）
- Mastodon 側の会話ログをブートストラップとして渡す＆SQLite でスレッド情報を保持
- プロンプトを `config/prompts.json` で管理
- Docker / Docker Compose 対応
- `cargo fmt` / `clippy` による整形・静的解析

---

## 📦 Repo structure

```
mast_gpt_bot/
├─ Cargo.toml
├─ Cargo.lock
├─ .env.example                 # 環境変数サンプル
├─ Dockerfile
├─ docker-compose.yml
├─ config/
│  └─ prompts.json              # システム/ユーザー向けテンプレ群（Vec<ChatMessage>）
└─ src/
   ├─ main.rs                   # 起動：通知ストリーム＋定期 free toot
   ├─ config/                   # BotConfig / .env ローディング
   │  ├─ bot_config.rs
   │  ├─ env_parsing.rs
   │  ├─ redacted.rs
   │  └─ visibility.rs
   ├─ conversation_store.rs     # SQLite でスレッド毎の previous_response_id を保存
   ├─ mastodon.rs               # Mastodon API 型＋投稿ユーティリティ
   ├─ notification_stream/      # Streaming API リスナー
   │  ├─ connection.rs
   │  ├─ context.rs
   │  ├─ handler.rs
   │  └─ rate_limit.rs
   ├─ openai_api/
   │  ├─ free_toot.rs           # 自由トゥート生成（JST時刻を system で注入）
   │  ├─ prompts.rs             # prompts.json のローディング
   │  ├─ reply/                 # メンション返信生成
   │  │  ├─ message_builder.rs
   │  │  ├─ parrot_check.rs
   │  │  ├─ search.rs
   │  │  └─ time.rs
   │  ├─ stream.rs              # Responses API 呼び出し
   │  └─ types.rs               # リクエスト/レスポンス型 & tools（web_search_preview）
   └─ util.rs                   # HTML 除去、URL 正規化、文字数トリム
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
# Mastodon
MASTODON_BASE_URL=https://your.instance.example
MASTODON_ACCESS_TOKEN=xxxxxx
MASTODON_POST_VISIBILITY=unlisted   # 公開範囲 (public/unlisted/private/direct)
MASTODON_CHAR_LIMIT=500             # インスタンスの文字数上限

# OpenAI (Responses API)
OPENAI_API_KEY=sk-xxxx
OPENAI_MODEL=gpt-4.1-mini            # free toot 用などベースモデル
OPENAI_REPLY_MODEL=gpt-4.1-mini      # 返信用モデル（省略時は上記デフォルト）

# Prompts / 状態ファイル
PROMPTS_PATH=config/prompts.json     # プロンプトの場所
BOT_DB_PATH=bot_state.sqlite         # previous_response_id を保存する SQLite

# 動作チューニング
REPLY_TEMPERATURE=0.6
FREE_TOOT_TEMPERATURE=0.7
FREE_TOOT_INTERVAL_SECS=3600         # 自由トゥート間隔（秒）
REPLY_MIN_INTERVAL_MS=1000           # リプライ時の最小待機（ミリ秒）

# Streaming（通常は /api/v1/streaming 推測で OK）
# MASTODON_STREAMING_URL=wss://your.instance.example/api/v1/streaming

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
