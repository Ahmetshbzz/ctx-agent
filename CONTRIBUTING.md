# Contributing to ctx-agent

Thanks for contributing.

## Development setup

Requirements:
- Rust stable toolchain
- Node.js 18+

Install and build:

```bash
cargo build
cd mcp-server && npm install && npm run build
```

## Branching and commits

- Create a feature branch from `main`.
- Keep commits focused and small.
- Use clear commit messages (recommended: Conventional Commits).

Examples:
- `feat: add security guard output to ctx_status`
- `fix: handle missing project path in MCP tool`

## Pull request process

1. Sync with latest `main`.
2. Run checks locally:

```bash
cargo test
cargo clippy --all-targets -- -D warnings
cd mcp-server && npm run build
```

3. Update docs when behavior changes.
4. Open a PR using the template.

## Coding standards

- Keep changes minimal and targeted.
- Prefer deterministic behavior for agent-facing output.
- Avoid introducing breaking CLI/MCP changes without migration notes.
- Do not include secrets or credentials.

## Reporting issues

- Use issue templates for bugs and feature requests.
- Provide exact steps, expected behavior, and actual behavior.
- Include environment details (OS, Rust version, Node version, `ctx-agent --version`).
