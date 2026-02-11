# Agent Workflow with ctx-agent

## Goal

Use `ctx-agent` as the first context layer so each agent session starts with project memory instead of blind file traversal.

## Baseline Workflow

1. Project overview
- `ctx_status`
- If this is the first run (`knowledge_notes = 0`), `ctx_status` auto-bootstraps and stores an overview note
- `ctx_overview` (explicit project brief generation)
- `ctx_map` (optional for structure-heavy tasks)

2. Risk check before edits
- `ctx_warnings`
- `ctx_blast_radius` on candidate files

3. Target discovery
- `ctx_query "<feature-or-bug-keyword>"`
- Run multiple focused terms instead of a single broad query

4. Editing loop
- Edit code
- `ctx_scan`
- Re-check with `ctx_query` or `ctx_blast_radius`

5. Session closure
- Add explicit rationale via `ctx_learn` for non-obvious architectural choices

## Heuristics

- If `ctx_query` returns too many hits, split by domain term (`auth`, `session`, `token`, etc.).
- If `ctx_warnings` reports large files, prefer incremental extraction/refactor before feature additions.
- If `ctx_decisions` is rich, use it early to avoid reintroducing previously rejected patterns.

## Command Equivalents (CLI)

```bash
ctx-agent -p /path/to/project status --json
ctx-agent -p /path/to/project warnings --json
ctx-agent -p /path/to/project query "auth" --json
ctx-agent -p /path/to/project blast-radius src/db/mod.rs --json
ctx-agent -p /path/to/project scan --json
```
