# Security Policy

## Supported versions

Only the latest `main` branch is supported for security fixes.

## Reporting a vulnerability

Do not open a public GitHub issue for security vulnerabilities.

Report privately using GitHub Security Advisories for this repository.
If advisories are unavailable in your environment, open a private channel with maintainers before public disclosure.

Include:
- Affected version/commit
- Reproduction steps
- Impact assessment
- Suggested fix (optional)

Maintainers should acknowledge reports within 72 hours.

## Scope

This policy covers:
- `ctx-agent` CLI
- MCP server integration (`mcp-server/`)
- Data handling in local SQLite storage

Out of scope:
- Third-party package vulnerabilities without a project-specific exploit path
- Misconfiguration in external systems
