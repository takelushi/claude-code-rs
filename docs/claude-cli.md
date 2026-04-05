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
