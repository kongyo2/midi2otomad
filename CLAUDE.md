# 単体で完結するGUIを備えたMIDI音MAD作成デスクトップアプリ

## 開発スタイル

TDD で開発する（探索 → Red → Green → Refactoring）。
KPI やカバレッジ目標が与えられたら、達成するまで試行する。
不明瞭な指示は質問して明確にする。

## Code Comments Policy

- Do NOT add comments explaining what was changed or why a change was made
- Comments like `// changed from X to Y` or `// updated for feature Z` are forbidden
- Only add comments when the logic itself is genuinely complex and non-obvious

## Web Fetch Strategy

When fetching web content, try methods in this order. Move to the next if the current one fails (e.g. 403, timeout, aborted):

1. **WebFetch tool** - Default. Try this first.
2. **curl fallback** - If WebFetch returns 403, retry with `curl -sL -A "claude-code/1.0" <url>`. Many 403s are caused by Cloudflare blocking the default `Claude-User` User-Agent.
3. `npx -y @mizchi/readability --format=md "<url>"` — extracts the main content (strips nav/ads/sidebars) and serializes to Markdown.
