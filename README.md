# mast_gpt_bot

Mastodon の通知ストリームでメンションを受け取り、OpenAI Responses API で返信する Rust 製 bot です。一定間隔で通常投稿も生成します。

返信では Mastodon 側の会話コンテキストと OpenAI の `previous_response_id` を使い、スレッドごとの会話状態を SQLite に保存します。必要に応じて OpenAI の `web_search_preview` ツールも利用できます。

## 主な機能

- Mastodon Streaming API の user ストリームに接続し、`mention` 通知だけを処理
- bot アカウントからのメンションを無視して返信ループを抑制
- 初回返信時に Mastodon の status context を取得し、スレッド文脈として OpenAI に渡す
- 2回目以降は SQLite に保存した `previous_response_id` を使って会話を継続
- 時間帯に応じた自由トゥートを定期生成
- `config/prompts.json` で返信用・自由トゥート用プロンプトを管理
- OpenAI Responses API の `web_search_preview` に対応
- ローカル実行、Docker、Docker Compose に対応

## 構成

```text
.
├── Cargo.toml
├── Cargo.lock
├── Dockerfile
├── docker-compose.yml
├── .env.example
├── config/
│   └── prompts.json
└── src/
    ├── main.rs
    ├── config/
    ├── conversation_store.rs
    ├── mastodon.rs
    ├── notification_stream/
    ├── openai_api/
    └── util.rs
```

主要な役割は次の通りです。

- `src/main.rs`: 設定読み込み、通知ストリーム処理、自由トゥート処理を起動
- `src/config/`: `.env` から `BotConfig` を生成
- `src/notification_stream/`: WebSocket 接続、通知イベント処理、返信レート制御
- `src/openai_api/`: Responses API 呼び出し、返信生成、自由トゥート生成、プロンプト読み込み
- `src/conversation_store.rs`: SQLite にスレッドごとの `last_response_id` を保存
- `src/mastodon.rs`: Mastodon API の context 取得、返信投稿、通常投稿
- `src/util.rs`: HTML 除去、URL/Markdownリンク正規化、文字数調整

## 必要なもの

- Rust stable
- Mastodon アカウントとアクセストークン
- OpenAI API key
- Docker / Docker Compose 任意

`rust-toolchain.toml` で `rustfmt` と `clippy` コンポーネントを指定しています。

## セットアップ

`.env.example` をコピーして `.env` を作成します。

```bash
cp .env.example .env
```

最低限、次の値を設定してください。

```dotenv
MASTODON_BASE_URL=https://your.instance.example
MASTODON_ACCESS_TOKEN=xxxxxxxxxxxxxxxxxxxx

OPENAI_API_KEY=sk-xxxxxxxxxxxxxxxxxxxx
OPENAI_MODEL=gpt-4.1-mini
OPENAI_REPLY_MODEL=gpt-4.1-mini
PROMPTS_PATH=config/prompts.json
```

`PROMPTS_PATH` は `.env.example` と同じく `config/prompts.json` を指定しておくのがおすすめです。現在のプロンプトローダーは環境変数 `PROMPTS_PATH` を直接参照します。

## 環境変数

| 変数 | 必須 | 既定値 | 内容 |
| --- | --- | --- | --- |
| `MASTODON_BASE_URL` | yes | なし | Mastodon インスタンスの URL |
| `MASTODON_ACCESS_TOKEN` | yes | なし | Mastodon API トークン |
| `OPENAI_API_KEY` | yes | なし | OpenAI API key |
| `OPENAI_MODEL` | yes | なし | 自由トゥート生成に使うモデル |
| `OPENAI_REPLY_MODEL` | no | `gpt-4.1-mini` | 返信生成に使うモデル |
| `PROMPTS_PATH` | no | 明示設定推奨 | プロンプト JSON のパス |
| `BOT_DB_PATH` | no | `bot_state.sqlite` | 会話状態を保存する SQLite ファイル |
| `MASTODON_STREAMING_URL` | no | `MASTODON_BASE_URL` から推測 | Streaming API の WebSocket URL |
| `MASTODON_POST_VISIBILITY` | no | `unlisted` | 自由トゥートの公開範囲 |
| `MASTODON_CHAR_LIMIT` | no | `500` | 自由トゥートの文字数上限 |
| `FREE_TOOT_INTERVAL_SECS` | no | `3600` | 自由トゥート間隔 |
| `REPLY_MIN_INTERVAL_MS` | no | `3000` | 返信処理前の最小待機時間 |
| `REPLY_TEMPERATURE` | no | `0.7` | 返信生成の temperature |
| `FREE_TOOT_TEMPERATURE` | no | `0.8` | 自由トゥート生成の temperature |
| `ENABLE_WEB_SEARCH` | no | `false` | `web_search_preview` を有効化 |

`MASTODON_STREAMING_URL` を省略すると、`https://example.com` は `wss://example.com/api/v1/streaming` に、`http://example.com` は `ws://example.com/api/v1/streaming` に変換されます。

`MASTODON_POST_VISIBILITY` は `public`、`unlisted`、`private`、`direct` が利用できます。返信投稿では元投稿の visibility を引き継ぎ、自由トゥートではこの設定を使います。

## プロンプト

`config/prompts.json` は次のキーを持つ JSON です。各値は OpenAI に渡す `role` / `content` 形式のメッセージ配列です。

- `free_toot_morning`
- `free_toot_day`
- `free_toot_night`
- `reply_with_context`
- `reply_without_context`

自由トゥートでは JST の現在時刻から朝・昼・夕方・夜を判定します。夕方は `free_toot_day` を流用します。実行時には JST 現在時刻を system instruction として追加します。

返信プロンプトでは、テンプレート内に `{{USER_TEXT}}` と `{{CONTEXT}}` を置くと実際のメンション本文と会話コンテキストに置換されます。プレースホルダーがない場合は、コード側で user メッセージや context を追加します。

## 実行

ローカルで実行する場合:

```bash
cargo run
```

Docker で実行する場合:

```bash
docker build -t mast-gpt-bot:latest .
docker run --rm --env-file .env mast-gpt-bot:latest
```

Docker Compose で実行する場合:

```bash
docker compose up --build
```

Compose では外部公開ポートは設定していません。bot は Mastodon と OpenAI に outbound 接続します。

## 動作の流れ

1. `.env` から設定を読み込みます。
2. SQLite DB を開き、`conversations` テーブルを初期化します。
3. Mastodon Streaming API に `stream=user` で接続します。
4. `notification` イベントのうち `type == "mention"` のみ処理します。
5. Mastodon の status context を取得し、スレッドルート ID を `thread_key` にします。
6. SQLite から `previous_response_id` を取得します。
7. OpenAI Responses API で返信を生成します。
8. Mastodon に返信を投稿し、最新の response id を SQLite に保存します。
9. 別タスクで `FREE_TOOT_INTERVAL_SECS` ごとに自由トゥートを生成・投稿します。

WebSocket 接続が切れた場合は 5 秒後に再接続します。

## Web 検索

`ENABLE_WEB_SEARCH=true` の場合、返信生成と自由トゥート生成で OpenAI の `web_search_preview` ツールを渡します。

また、返信本文にリリースノート、変更点、バージョン番号などの語が含まれる場合は、`ENABLE_WEB_SEARCH` が `false` でも検索ツールを強制的に有効化します。この場合は短い箇条書きと出典ドメインを返すよう追加指示を入れます。

## 開発

整形:

```bash
cargo fmt --all
```

整形チェック:

```bash
cargo fmt --all -- --check
```

テスト:

```bash
cargo test
```

Lint:

```bash
cargo clippy -- -D warnings
```

## 運用メモ

- SQLite は `BOT_DB_PATH` に作成され、WAL モードで利用されます。
- `bot_state.sqlite*` は実行時状態なので、通常はリポジトリに含めない運用が安全です。
- `.env` には API key やアクセストークンが入るため公開しないでください。
- インスタンスによって Streaming API の URL が異なる場合は `MASTODON_STREAMING_URL` を明示してください。
- OpenAI API の失敗や Mastodon 投稿失敗はログに出力されます。

## ライセンス

MIT License です。詳細は [LICENSE](LICENSE) を参照してください。
