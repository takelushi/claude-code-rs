# Claude CLI 挙動・制約メモ

実装中に判明した Claude Code CLI の挙動・制約を記録する。

## ANSI エスケープシーケンスの混入

`--output-format json` / `stream-json` の stdout に ANSI エスケープシーケンス（例: `\x1b[?1004l`）が混入する場合がある。JSON パース前にストリップが必要。

## コンテキスト最小化構成

以下のオプションを組み合わせることで、Claude Code が注入するコンテキストを最小化できる（約100トークンまで削減）:

| オプション | 削減対象 |
| --- | --- |
| `--system-prompt ''` | デフォルトのシステムプロンプトを空にする |
| `--setting-sources ''` | ユーザー/プロジェクト/ローカル設定の読み込みをスキップ |
| `--strict-mcp-config` | `--mcp-config` 以外の MCP 設定を無視する |
| `--mcp-config '{"mcpServers":{}}'` | MCP サーバーをゼロにする |
| `--tools ''` | ビルトインツール定義を全て除外する（約9000トークン削減） |
| `--disable-slash-commands` | スラッシュコマンド (skill) を無効にする |

残る約100トークンは Claude Code がハードコードで注入する基盤プロンプト（`currentDate` + `You are a Claude agent...`）。

## stream-json は --verbose が必須

`--output-format stream-json` は `--verbose` を併用しないとエラーになる。

## --max-turns の挙動

`--max-turns 1` を指定すると、ツール使用なしの1回応答で停止する。E2E テストで課金を抑えるのに有用。

## --no-session-persistence

セッションをディスクに保存しない。`--resume` によるセッション再開が不要な場合（ライブラリからの単発呼び出し等）に使う。

## stream-json のイベント型

`--output-format stream-json --verbose` で出力される NDJSON の各イベント:

| type | subtype / content type | 内容 |
| --- | --- | --- |
| `system` | `init` | セッション初期化情報（session_id, model 等） |
| `system` | `hook_started` / `hook_response` | hook の実行（ライブラリではスキップ） |
| `assistant` | content[].type = `thinking` | モデルの思考過程 |
| `assistant` | content[].type = `text` | テキスト応答チャンク |
| `assistant` | content[].type = `tool_use` | ツール呼び出し |
| `user` | content[].type = `tool_result` | ツール実行結果 |
| `rate_limit_event` | — | レートリミット情報 |
| `result` | `success` | 最終結果（`--output-format json` と同じ構造） |
| `stream_event` | `content_block_delta` | トークン単位のストリーミングチャンク（`--include-partial-messages` 時のみ） |

### content 配列の複数要素

1つの `assistant` / `user` イベントの `.message.content[]` に複数ブロックが含まれる場合がある。ライブラリでは各要素を個別の `StreamEvent` として yield する。

### stream_event 型（リアルタイムストリーミング）

`--include-partial-messages` を付けると、上記イベントに加えて `stream_event` 型のイベントが送信される。これは Anthropic Messages API の SSE イベントをラップしたもので、トークン単位のリアルタイムストリーミングを実現する。

構造: `{"type": "stream_event", "event": {"type": "content_block_delta", "delta": {...}}}`

| event.delta.type | 内容 |
| --- | --- |
| `text_delta` | テキストチャンク（`.delta.text`） |
| `thinking_delta` | 思考チャンク（`.delta.thinking`） |

`--include-partial-messages` なしの場合、テキスト応答は `assistant` イベントとして完成後にまとめて1回送信される。リアルタイム表示が必要な場合は `--include-partial-messages` を有効にし、`stream_event` の `text_delta` / `thinking_delta` を使う。

### --include-partial-messages

このオプションを付けると、`stream_event` 型のイベントが追加で送信され、テキストがトークン単位のチャンクでリアルタイムにストリーミングされる。デフォルト（なし）では `assistant` イベントとして完成したメッセージが送信される。
