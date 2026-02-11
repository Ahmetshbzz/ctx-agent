# Real-World MCP Validation


Project under test: `/path/to/project`
Validation mode: MCP tools only (`ctx_init`, `ctx_status`, `ctx_scan`, `ctx_query`, `ctx_blast_radius`, `ctx_warnings`, `ctx_decisions`)

## Summary

`ctx-agent` provided high-value project context to an agent without running the target application.
The tested project is non-trivial and polyglot:

- Files: 483
- Lines: 77,235
- Symbols: 3,169
- Dependencies: 1,428
- Decisions tracked: 171

## MCP Calls and Outcomes

1. `ctx_init`
- Result: already initialized, re-scan completed
- Re-scan time: ~3.7s
- Changed files analyzed: 0 (483 unchanged)

2. `ctx_status`
- Returned full dashboard with language distribution:
- TypeScript: 260 files
- Go: 131 files
- JSON: 24 files
- Swift: 23 files
- Markdown: 14 files

3. `ctx_query "auth"`
- Result count: 50 (capped)
- Immediate hits in backend auth helpers, JWT paths, telegram auth store, panel auth context

4. `ctx_query "payment"`
- Result count: 28
- Immediate hits in payment UI flow components and forms

5. `ctx_warnings`
- Total warnings: 13
- All warnings were large files (no fragile/dead warnings in this run)

6. `ctx_blast_radius apps/core/internal/admin/handler.go`
- Imports: 19
- Imported by: 0
- Risk: low

7. `ctx_decisions`
- Decisions available: 171
- Practical value: fast architectural context recovery from commit history

## What This Means for Agents

In a fresh session, an agent can quickly recover:

- Current architecture shape
- Relevant code areas for a task keyword
- High-risk or oversized files before editing
- Historical intent from commit-derived decisions

This reduces blind exploration and improves first-attempt edit quality.

## Known Practical Limits

- Blast radius quality depends on import extraction quality per language.
- Query output is capped to top matches; agents should run multiple targeted queries.
- Health signals are structural; they do not replace runtime testing.

## Recommended MCP Startup Sequence

For every new agent session on a repo:

1. `ctx_status`
2. `ctx_warnings`
3. 2-3 targeted `ctx_query` calls
4. `ctx_blast_radius` for candidate files before edits
5. `ctx_scan` after edits

