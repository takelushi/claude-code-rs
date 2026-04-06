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
| `stream_event` | （各種、下記参照） | Anthropic Messages API の SSE イベント（`--include-partial-messages` 時のみ） |

### content 配列の複数要素

1つの `assistant` / `user` イベントの `.message.content[]` に複数ブロックが含まれる場合がある。ライブラリでは各要素を個別の `StreamEvent` として yield する。

### assistant イベントと stream_event の関係

`--include-partial-messages` を有効にすると、`assistant` イベント（完成メッセージ）と `stream_event`（トークン単位チャンク）の両方が送信される。同じテキストが2回届くため、ライブラリでは以下のように区別する:

| ソース | StreamEvent バリアント | 用途 |
| --- | --- | --- |
| `stream_event` / `text_delta` | `Text` | リアルタイム表示 |
| `stream_event` / `thinking_delta` | `Thinking` | リアルタイム表示 |
| `assistant` / text | `AssistantText` | 完成テキスト取得 |
| `assistant` / thinking | `AssistantThinking` | 完成テキスト取得 |

### stream_event 型（リアルタイムストリーミング）

`--include-partial-messages` を付けると、Anthropic Messages API の SSE イベントが `stream_event` 型でラップされて送信される。

構造: `{"type": "stream_event", "event": {"type": "<event_type>", ...}}`

#### イベント型一覧

| event.type | StreamEvent バリアント | 内容 |
| --- | --- | --- |
| `message_start` | `MessageStart` | メッセージ開始（モデル名、ID） |
| `content_block_start` | `ContentBlockStart` | ブロック開始（index, block_type） |
| `content_block_delta` | 各 delta バリアント | トークン単位チャンク（下記参照） |
| `content_block_stop` | `ContentBlockStop` | ブロック終了（index） |
| `message_delta` | `MessageDelta` | stop_reason 等 |
| `message_stop` | `MessageStop` | メッセージ完了 |
| `ping` | `Ping` | キープアライブ |
| `error` | `Error` | エラー通知 |

#### content_block_delta の delta type

| event.delta.type | StreamEvent バリアント | 内容 |
| --- | --- | --- |
| `text_delta` | `Text` | テキストチャンク（`.delta.text`） |
| `thinking_delta` | `Thinking` | 思考チャンク（`.delta.thinking`） |
| `input_json_delta` | `InputJsonDelta` | ツール入力の部分 JSON（`.delta.partial_json`） |
| `signature_delta` | `SignatureDelta` | thinking の署名（`.delta.signature`） |
| `citations_delta` | `CitationsDelta` | 引用情報（`.delta.citation`） |

#### イベント送信順序

```plain
message_start
→ content_block_start (index=0, type=thinking)
→ thinking_delta (複数回)
→ signature_delta
→ content_block_stop (index=0)
→ content_block_start (index=1, type=text)
→ text_delta (複数回)
→ content_block_stop (index=1)
→ message_delta (stop_reason)
→ message_stop
```

### --include-partial-messages

このオプションを付けると、`stream_event` 型のイベントが追加で送信され、テキストがトークン単位のチャンクでリアルタイムにストリーミングされる。デフォルト（なし）では `assistant` イベントとして完成したメッセージのみが送信される。

リアルタイム表示が必要な場合は `include_partial_messages(true)` を有効にし、`Text` / `Thinking` バリアントを使う。完成テキストだけ必要な場合は `AssistantText` / `AssistantThinking` を使う。
