#!/usr/bin/env node

import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import { execFileSync, execSync } from "node:child_process";
import { resolve, dirname, join } from "node:path";
import { existsSync, readdirSync, readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { z } from "zod";

// ── Locate ctx binary ──────────────────────────────────────────────

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

function findCtxBinary(): string {
    // 1. CTX_BIN env var
    if (process.env.CTX_BIN && existsSync(process.env.CTX_BIN)) {
        return process.env.CTX_BIN;
    }

    // 2. Sibling target/release or target/debug (when inside ctx project)
    const projectRoot = resolve(__dirname, "../..");
    const release = resolve(projectRoot, "target/release/ctx");
    const debug = resolve(projectRoot, "target/debug/ctx");
    if (existsSync(release)) return release;
    if (existsSync(debug)) return debug;

    // 3. On PATH
    try {
        const which = execSync("which ctx", { encoding: "utf-8" }).trim();
        if (which) return which;
    } catch {
        // not on path
    }

    throw new Error(
        "ctx binary not found. Set CTX_BIN env var or build with: cargo build --release"
    );
}

const CTX_BIN = findCtxBinary();

// ── Auto-init: ensure project is initialized ────────────────────────

function ensureInitialized(projectPath: string): void {
    try {
        execSync(`"${CTX_BIN}" -p "${projectPath}" status --json`, {
            encoding: "utf-8",
            timeout: 20_000,
            env: { ...process.env, NO_COLOR: "1" },
            maxBuffer: 10 * 1024 * 1024,
        });
        return;
    } catch {
        execSync(`"${CTX_BIN}" -p "${projectPath}" init`, {
            encoding: "utf-8",
            timeout: 60_000,
            env: { ...process.env, NO_COLOR: "1" },
            maxBuffer: 10 * 1024 * 1024,
        });
    }
}

// ── Helper: run ctx CLI command ─────────────────────────────────────

interface CtxResult {
    success: boolean;
    output: string;
}

function runCtxArgv(args: string[], projectPath: string, skipAutoInit = false): CtxResult {
    // Auto-initialize if needed (skip for init command itself)
    if (!skipAutoInit) {
        try {
            ensureInitialized(projectPath);
        } catch {
            // init failed, continue anyway — the actual command will show the error
        }
    }

    try {
        const output = execFileSync(CTX_BIN, ["-p", projectPath, ...args], {
            encoding: "utf-8",
            timeout: 30_000,
            env: { ...process.env, NO_COLOR: "1" },
            maxBuffer: 10 * 1024 * 1024,
        });
        // Strip ANSI escape codes
        const clean = output.replace(/\x1B\[[0-9;]*[a-zA-Z]/g, "").trim();
        return { success: true, output: clean };
    } catch (err: unknown) {
        const message = err instanceof Error ? err.message : String(err);
        return { success: false, output: `Error: ${message}` };
    }
}

function safeRead(filePath: string, maxChars = 10_000): string {
    if (!existsSync(filePath)) return "";
    try {
        const content = readFileSync(filePath, "utf-8");
        return content.slice(0, maxChars);
    } catch {
        return "";
    }
}

function compactParagraph(text: string, maxLen = 260): string {
    const cleaned = text
        .replace(/```[\s\S]*?```/g, " ")
        .replace(/[#>*`|]/g, " ")
        .replace(/\s+/g, " ")
        .trim();
    if (!cleaned) return "";
    return cleaned.length > maxLen ? `${cleaned.slice(0, maxLen - 3)}...` : cleaned;
}

function stripMarkdownInline(text: string): string {
    return text
        .replace(/!\[[^\]]*\]\([^)]+\)/g, " ")
        .replace(/\[([^\]]+)\]\([^)]+\)/g, "$1")
        .replace(/`([^`]+)`/g, "$1")
        .replace(/[*_~]/g, " ");
}

function isBadPurposeCandidate(rawLine: string, cleanedLine: string): boolean {
    const raw = rawLine.trim();
    const cleaned = cleanedLine.trim();
    if (!raw || !cleaned) return true;

    const lowerRaw = raw.toLowerCase();
    const lowerClean = cleaned.toLowerCase();

    if (raw.startsWith("#")) return true;
    if (raw.startsWith("![")) return true;
    if (lowerRaw.includes("img.shields.io")) return true;
    if (lowerRaw.includes("shields.io")) return true;
    if (lowerClean.includes("license")) return true;
    if (lowerClean.includes("build status")) return true;
    if (lowerClean.includes("coverage")) return true;
    if (lowerClean.includes("badge")) return true;
    if (!/[a-zA-Z]/.test(cleaned)) return true;
    if (cleaned.length < 40) return true;
    return false;
}

function extractPurposeFromReadme(readme: string): string {
    const lines = readme.split(/\r?\n/);
    for (const line of lines) {
        const stripped = stripMarkdownInline(line);
        const cleaned = compactParagraph(stripped, 260);
        if (!isBadPurposeCandidate(line, cleaned)) {
            return compactParagraph(cleaned, 220);
        }
    }

    const paragraphs = readme
        .split(/\r?\n\r?\n/)
        .map((p) => compactParagraph(stripMarkdownInline(p), 260))
        .filter((p) => p.length >= 60 && /[a-zA-Z]/.test(p));
    return paragraphs[0] ? compactParagraph(paragraphs[0], 220) : "";
}

function detectTopModules(projectPath: string): string[] {
    try {
        return readdirSync(projectPath, { withFileTypes: true })
            .filter((d) => d.isDirectory())
            .map((d) => d.name)
            .filter((name) => !name.startsWith("."))
            .slice(0, 8);
    } catch {
        return [];
    }
}

interface ProjectOverview {
    bullets: string[];
    sources: string[];
    note: string;
}

interface GuardReport {
    mode: "paranoid" | "off";
    touchedFiles: string[];
    sensitiveFiles: string[];
    requiredControls: string[];
    missingControls: string[];
    status: "pass" | "block";
    risk: "low" | "medium" | "high" | "critical";
}

function buildProjectOverview(projectPath: string): ProjectOverview {
    const sourceCandidates = [
        "README.md",
        "readme.md",
        "ARCHITECTURE.md",
        "architecture.md",
        "domain-integration.md",
        "bot.md",
        "apps/core/main.go",
        "apps/core/routes.go",
    ];
    const existingSources = sourceCandidates.filter((p) => existsSync(join(projectPath, p)));
    const docsCombined = existingSources
        .map((p) => safeRead(join(projectPath, p)))
        .filter(Boolean)
        .join("\n");
    const combinedLower = docsCombined.toLowerCase();

    const readme = safeRead(join(projectPath, "README.md")) || safeRead(join(projectPath, "readme.md"));
    const purposeLine = extractPurposeFromReadme(readme);

    const modules = detectTopModules(projectPath);
    const hasApps = existsSync(join(projectPath, "apps"));
    const hasCore = existsSync(join(projectPath, "apps/core"));
    const hasPanel = existsSync(join(projectPath, "apps/panel"));
    const hasClient = existsSync(join(projectPath, "apps/client"));
    const hasBot = existsSync(join(projectPath, "bot")) || combinedLower.includes("telegram");
    const hasWs =
        combinedLower.includes("websocket") ||
        combinedLower.includes("ws hub") ||
        combinedLower.includes("realtime");
    const hasTenant = combinedLower.includes("tenant") || combinedLower.includes("multi-tenant");
    const hasDomain = combinedLower.includes("domain") || combinedLower.includes("dns");
    const hasAuth =
        combinedLower.includes("auth") ||
        combinedLower.includes("jwt") ||
        combinedLower.includes("csrf") ||
        combinedLower.includes("totp");

    const bullets = [
        `1) Product purpose: ${purposeLine || "This repository is a production-oriented software platform with a modular architecture."}`,
        `2) Primary users: ${hasPanel ? "admin and operations teams via the panel" : "internal operators"}${hasClient ? ", plus tenant/client end users" : ""}${hasBot ? ", with bot-based remote operation support" : ""}.`,
        `3) Main modules: ${hasApps ? "multi-app workspace under `apps/`" : "monolithic project layout"}${hasCore ? " with a backend core service" : ""}${hasPanel ? ", admin panel" : ""}${hasClient ? ", and client frontend" : ""}.`,
        `4) Backend responsibility: central API routing, service orchestration, and business logic${hasTenant ? " with tenant-aware isolation" : ""}.`,
        `5) Critical runtime flow: request handling across auth, admin operations, and domain-specific endpoints via backend route registration.`,
        `6) Security posture: ${hasAuth ? "auth/session hardening with mechanisms like JWT/cookies/CSRF/TOTP and audit controls" : "access control and session management implemented in application services"}.`,
        `7) Operational flow: ${hasDomain ? "domain, DNS, and SSL-related lifecycle management appears to be integrated into platform workflows" : "deployment and runtime operations are documented in project-specific integration docs"}.`,
        `8) Realtime and integrations: ${hasWs ? "realtime event transport (WebSocket-style) is part of the platform architecture" : "realtime transport is not explicit in sampled docs"}${hasBot ? ", and bot integration is present for operational automation." : "."}`,
    ];

    const note = [
        "Project overview (auto-generated by ctx MCP):",
        "",
        ...bullets,
        "",
        `Sources: ${existingSources.join(", ") || "none detected"}`,
        modules.length ? `Top-level modules: ${modules.join(", ")}` : "",
    ]
        .filter(Boolean)
        .join("\n");

    return {
        bullets,
        sources: existingSources,
        note,
    };
}

function ensureOverviewNoteIfNeeded(projectPath: string): "saved" | "skipped" | "failed" {
    const statusJson = runCtxArgv(["status", "--json"], projectPath);
    if (!statusJson.success) return "failed";
    try {
        const parsed = JSON.parse(statusJson.output) as { knowledge_notes?: number };
        if ((parsed.knowledge_notes ?? 0) > 0) return "skipped";
    } catch {
        return "failed";
    }
    const overview = buildProjectOverview(projectPath);
    const saved = runCtxArgv(["learn", overview.note], projectPath);
    return saved.success ? "saved" : "failed";
}

function paranoidModeEnabled(): boolean {
    const value = (process.env.CTX_PARANOID ?? "1").toLowerCase();
    return !(value === "0" || value === "false" || value === "off");
}

function runGit(projectPath: string, args: string[]): string {
    try {
        return execFileSync("git", args, {
            cwd: projectPath,
            encoding: "utf-8",
            timeout: 15_000,
            env: { ...process.env, NO_COLOR: "1" },
            maxBuffer: 5 * 1024 * 1024,
        }).trim();
    } catch {
        return "";
    }
}

function hasBinary(name: string): boolean {
    try {
        execSync(`command -v ${name}`, { stdio: "ignore" });
        return true;
    } catch {
        return false;
    }
}

function runTextSearch(projectPath: string, pattern: string, maxResults = 60): string {
    const safeMax = Math.min(Math.max(maxResults, 1), 200);
    const rgArgs = [
        "-n",
        "-S",
        "--no-heading",
        "--max-count",
        String(safeMax),
        "--glob",
        "!**/node_modules/**",
        "--glob",
        "!**/dist/**",
        "--glob",
        "!**/target/**",
        "--glob",
        "!**/.git/**",
        pattern,
        ".",
    ];

    try {
        if (hasBinary("rg")) {
            const out = execFileSync("rg", rgArgs, {
                cwd: projectPath,
                encoding: "utf-8",
                timeout: 20_000,
                maxBuffer: 10 * 1024 * 1024,
            }).trim();
            return out || "No text matches found.";
        }
    } catch (err: unknown) {
        const message = err instanceof Error ? err.message : String(err);
        if (!message.includes("status 1")) {
            return `Text search error: ${message}`;
        }
        return "No text matches found.";
    }

    try {
        const out = execFileSync(
            "grep",
            [
                "-RIn",
                "--exclude-dir=node_modules",
                "--exclude-dir=dist",
                "--exclude-dir=target",
                "--exclude-dir=.git",
                "-m",
                String(safeMax),
                pattern,
                ".",
            ],
            {
                cwd: projectPath,
                encoding: "utf-8",
                timeout: 20_000,
                maxBuffer: 10 * 1024 * 1024,
            }
        ).trim();
        return out || "No text matches found.";
    } catch {
        return "No text matches found.";
    }
}

function getTouchedFiles(projectPath: string): string[] {
    const outputs = [
        runGit(projectPath, ["diff", "--name-only"]),
        runGit(projectPath, ["diff", "--name-only", "--cached"]),
        runGit(projectPath, ["ls-files", "--others", "--exclude-standard"]),
    ];
    const files = new Set<string>();
    for (const chunk of outputs) {
        for (const line of chunk.split(/\r?\n/)) {
            const trimmed = line.trim();
            if (trimmed) files.add(trimmed);
        }
    }
    return Array.from(files).sort();
}

function isSensitivePath(filePath: string): boolean {
    const p = filePath.toLowerCase();
    return /(auth|session|token|jwt|crypto|cipher|tls|oauth|password|secret|cookie|csrf|admin)/.test(
        p
    );
}

function hasRepoPattern(projectPath: string, pattern: string): boolean {
    try {
        execFileSync(
            "rg",
            [
                "--no-messages",
                "--glob",
                "!**/node_modules/**",
                "--glob",
                "!**/dist/**",
                "--glob",
                "!**/target/**",
                "-n",
                "-S",
                pattern,
                projectPath,
            ],
            { encoding: "utf-8", timeout: 15_000, maxBuffer: 5 * 1024 * 1024 }
        );
        return true;
    } catch {
        return false;
    }
}

function buildGuardReport(projectPath: string): GuardReport {
    if (!paranoidModeEnabled()) {
        return {
            mode: "off",
            touchedFiles: [],
            sensitiveFiles: [],
            requiredControls: [],
            missingControls: [],
            status: "pass",
            risk: "low",
        };
    }

    const touchedFiles = getTouchedFiles(projectPath);
    const sensitiveFiles = touchedFiles.filter(isSensitivePath);
    const requiredControls = [
        "refresh token rotation",
        "refresh token replay/reuse detection",
        "global revoke on token reuse",
        "rate limiting / throttling on auth endpoints",
        "security-focused tests for auth/session flows",
    ];

    const controls: Array<{ name: string; ok: boolean }> = [
        {
            name: "refresh token rotation",
            ok: hasRepoPattern(projectPath, "(rotate.?session|token.?rotation|refresh.?token)"),
        },
        {
            name: "refresh token replay/reuse detection",
            ok: hasRepoPattern(projectPath, "(reuse|replay|token.?family)"),
        },
        {
            name: "global revoke on token reuse",
            ok: hasRepoPattern(projectPath, "(revoke.?all|revokeall|invalidate.?all)"),
        },
        {
            name: "rate limiting / throttling on auth endpoints",
            ok: hasRepoPattern(projectPath, "(rate.?limit|throttle|too.?many.?requests)"),
        },
        {
            name: "security-focused tests for auth/session flows",
            ok: hasRepoPattern(projectPath, "(auth|session|token|refresh).*(test|spec)|(test|spec).*(auth|session|token|refresh)"),
        },
    ];

    const missingControls = controls.filter((c) => !c.ok).map((c) => c.name);

    if (sensitiveFiles.length === 0) {
        return {
            mode: "paranoid",
            touchedFiles,
            sensitiveFiles,
            requiredControls,
            missingControls: [],
            status: "pass",
            risk: touchedFiles.length > 0 ? "medium" : "low",
        };
    }

    return {
        mode: "paranoid",
        touchedFiles,
        sensitiveFiles,
        requiredControls,
        missingControls,
        status: missingControls.length === 0 ? "pass" : "block",
        risk: missingControls.length === 0 ? "high" : "critical",
    };
}

// ── Shared schema fragments ─────────────────────────────────────────

const ProjectPathSchema = z.object({
    project_path: z
        .string()
        .describe("Absolute path to the project root directory"),
});

// ── MCP Server ──────────────────────────────────────────────────────

const server = new McpServer({
    name: "ctx",
    version: "1.0.0",
});

// ── Tool: ctx_init ──────────────────────────────────────────────────

server.tool(
    "ctx_init",
    "Initialize ctx in a project directory. Creates a project-specific database in the global ctx store, scans all files, extracts symbols (functions, classes, structs), maps dependencies, and analyzes git history for decisions.",
    ProjectPathSchema.shape,
    async ({ project_path }) => {
        const { output } = runCtxArgv(["init"], project_path, true);
        return { content: [{ type: "text" as const, text: output }] };
    }
);

// ── Tool: ctx_status ────────────────────────────────────────────────

server.tool(
    "ctx_status",
    "Get project dashboard: total files, lines of code, symbols, dependencies, decisions, knowledge notes, and language breakdown. Always appends a compact project overview (purpose, users, modules, critical flows). If there are no knowledge notes yet, it automatically saves the first overview note.",
    ProjectPathSchema.shape,
    async ({ project_path }) => {
        const noteStatus = ensureOverviewNoteIfNeeded(project_path);
        const overview = buildProjectOverview(project_path);
        const guard = buildGuardReport(project_path);
        const { output } = runCtxArgv(["status"], project_path);
        const guardLines = [
            "Security guard:",
            `Mode: ${guard.mode}`,
            `Status: ${guard.status.toUpperCase()} (risk: ${guard.risk})`,
            `Touched files: ${guard.touchedFiles.length}`,
            `Sensitive files: ${guard.sensitiveFiles.length}`,
        ];
        if (guard.sensitiveFiles.length > 0) {
            guardLines.push("Sensitive paths:");
            guard.sensitiveFiles.slice(0, 10).forEach((f) => guardLines.push(`- ${f}`));
        }
        if (guard.status === "block") {
            guardLines.push("Missing controls:");
            guard.missingControls.forEach((m) => guardLines.push(`- ${m}`));
        }

        const suffix = [
            "",
            "",
            "Project overview:",
            ...overview.bullets,
            "",
            `Overview note status: ${noteStatus}`,
            "",
            ...guardLines,
        ].join("\n");
        return { content: [{ type: "text" as const, text: `${output}${suffix}` }] };
    }
);

// ── Tool: ctx_map ───────────────────────────────────────────────────

server.tool(
    "ctx_map",
    "Display a structured codebase map showing the directory tree with file counts, line counts, and language breakdown per directory. Ideal for understanding project structure at a glance.",
    ProjectPathSchema.shape,
    async ({ project_path }) => {
        const { output } = runCtxArgv(["map"], project_path);
        return { content: [{ type: "text" as const, text: output }] };
    }
);

// ── Tool: ctx_scan ──────────────────────────────────────────────────

server.tool(
    "ctx_scan",
    "Re-scan the project incrementally. Only analyzes files whose content hash has changed. Updates symbols, dependencies, and the full-text search index.",
    ProjectPathSchema.shape,
    async ({ project_path }) => {
        const { output } = runCtxArgv(["scan"], project_path);
        return { content: [{ type: "text" as const, text: output }] };
    }
);

// ── Tool: ctx_query ─────────────────────────────────────────────────

server.tool(
    "ctx_query",
    "Full-text search across all symbols (functions, classes, structs, enums, etc.) using FTS5. Returns matching symbols with their full signatures and file locations. Supports partial matching.",
    {
        ...ProjectPathSchema.shape,
        query: z
            .string()
            .describe(
                "Search query — supports partial matches (e.g. 'parse', 'Database', 'init')"
            ),
    },
    async ({ project_path, query }) => {
        const { output } = runCtxArgv(["query", query], project_path);
        if (!output.includes("No results found.")) {
            return { content: [{ type: "text" as const, text: output }] };
        }
        const fallback = runTextSearch(project_path, query, 60);
        const text = [
            output,
            "",
            "Text search fallback (ripgrep/grep):",
            fallback,
        ].join("\n");
        return { content: [{ type: "text" as const, text }] };
    }
);

// ── Tool: ctx_blast_radius ──────────────────────────────────────────

server.tool(
    "ctx_blast_radius",
    "Analyze the blast radius of changing a specific file. Shows: what the file imports, what files depend on it, and the full transitive impact graph. Includes a risk assessment (low/medium/high/critical).",
    {
        ...ProjectPathSchema.shape,
        file_path: z
            .string()
            .describe("Relative path to the file (e.g. 'src/db/mod.rs')"),
    },
    async ({ project_path, file_path }) => {
        const { output } = runCtxArgv(["blast-radius", file_path], project_path);
        return { content: [{ type: "text" as const, text: output }] };
    }
);

// ── Tool: ctx_decisions ─────────────────────────────────────────────

server.tool(
    "ctx_decisions",
    "List all recorded architectural decisions. Includes decisions auto-extracted from conventional commits (feat/fix/refactor/breaking) and manually added entries.",
    ProjectPathSchema.shape,
    async ({ project_path }) => {
        const { output } = runCtxArgv(["decisions"], project_path);
        return { content: [{ type: "text" as const, text: output }] };
    }
);

// ── Tool: ctx_learn ─────────────────────────────────────────────────

server.tool(
    "ctx_learn",
    "Store a knowledge note about the project. Use this to record architectural insights, gotchas, design rationale, or any context that would help future development. Optionally link to a specific file.",
    {
        ...ProjectPathSchema.shape,
        note: z.string().describe("Knowledge note to record"),
        file: z
            .string()
            .optional()
            .describe("Optional: related file path for context"),
    },
    async ({ project_path, note, file }) => {
        const args = file ? ["learn", note, "--file", file] : ["learn", note];
        const { output } = runCtxArgv(args, project_path);
        return { content: [{ type: "text" as const, text: output }] };
    }
);

// ── Tool: ctx_warnings ──────────────────────────────────────────────

server.tool(
    "ctx_warnings",
    "Show codebase health warnings: fragile files (high churn + many dependents), large files (>500 lines), and potentially dead code (no commits, no dependents). Helps prioritize refactoring.",
    ProjectPathSchema.shape,
    async ({ project_path }) => {
        const { output } = runCtxArgv(["warnings"], project_path);
        return { content: [{ type: "text" as const, text: output }] };
    }
);

// ── Tool: ctx_overview ──────────────────────────────────────────────

server.tool(
    "ctx_overview",
    "Build an agent-ready project overview (purpose, users, modules, critical flows) from repository docs and structure. Also stores this overview as a knowledge note when none exists yet, unless disabled.",
    {
        ...ProjectPathSchema.shape,
        save_note: z
            .boolean()
            .optional()
            .describe("When true (default), save the generated overview into knowledge notes"),
    },
    async ({ project_path, save_note }) => {
        const overview = buildProjectOverview(project_path);
        let saved = "skipped";
        if (save_note !== false) {
            saved = ensureOverviewNoteIfNeeded(project_path);
        }

        const text = [
            "Project overview:",
            "",
            ...overview.bullets,
            "",
            `Sources: ${overview.sources.join(", ") || "none detected"}`,
            `Knowledge note: ${saved}`,
        ].join("\n");
        return { content: [{ type: "text" as const, text }] };
    }
);

// ── Tool: ctx_guard ─────────────────────────────────────────────────

server.tool(
    "ctx_guard",
    "Run paranoid security guard checks. If auth/session/token/crypto-related files are touched, this gate can return BLOCK unless critical controls are present (rotation, replay detection, global revoke, rate limiting, and tests).",
    ProjectPathSchema.shape,
    async ({ project_path }) => {
        const guard = buildGuardReport(project_path);
        const lines = [
            "Security guard report:",
            `Mode: ${guard.mode}`,
            `Status: ${guard.status.toUpperCase()}`,
            `Risk: ${guard.risk}`,
            `Touched files: ${guard.touchedFiles.length}`,
            `Sensitive files: ${guard.sensitiveFiles.length}`,
        ];
        if (guard.sensitiveFiles.length > 0) {
            lines.push("Sensitive paths:");
            guard.sensitiveFiles.forEach((f) => lines.push(`- ${f}`));
        }
        lines.push("Required controls:");
        guard.requiredControls.forEach((c) => lines.push(`- ${c}`));
        if (guard.missingControls.length > 0) {
            lines.push("Missing controls:");
            guard.missingControls.forEach((m) => lines.push(`- ${m}`));
        }
        return { content: [{ type: "text" as const, text: lines.join("\n") }] };
    }
);

// ── Tool: ctx_grep ──────────────────────────────────────────────────

server.tool(
    "ctx_grep",
    "Fast text search across the repository using ripgrep (fallback: grep). Useful when a symbol query misses strings, routes, handlers, or comments.",
    {
        ...ProjectPathSchema.shape,
        pattern: z.string().describe("Text or regex pattern to search"),
        max_results: z
            .number()
            .int()
            .min(1)
            .max(200)
            .optional()
            .describe("Maximum number of matches to return (default: 60)"),
    },
    async ({ project_path, pattern, max_results }) => {
        const output = runTextSearch(project_path, pattern, max_results ?? 60);
        return { content: [{ type: "text" as const, text: output }] };
    }
);

// ── Start server ────────────────────────────────────────────────────

async function main(): Promise<void> {
    const transport = new StdioServerTransport();
    await server.connect(transport);
}

main().catch((err: Error) => {
    console.error("ctx MCP server fatal error:", err.message);
    process.exit(1);
});
