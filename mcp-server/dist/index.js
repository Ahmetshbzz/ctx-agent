#!/usr/bin/env node
import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import { execSync } from "node:child_process";
import { resolve, dirname } from "node:path";
import { existsSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { z } from "zod";
// ── Locate ctx binary ──────────────────────────────────────────────
const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
function findCtxBinary() {
    // 1. CTX_BIN env var
    if (process.env.CTX_BIN && existsSync(process.env.CTX_BIN)) {
        return process.env.CTX_BIN;
    }
    // 2. Sibling target/release or target/debug (when inside ctx project)
    const projectRoot = resolve(__dirname, "../..");
    const release = resolve(projectRoot, "target/release/ctx");
    const debug = resolve(projectRoot, "target/debug/ctx");
    if (existsSync(release))
        return release;
    if (existsSync(debug))
        return debug;
    // 3. On PATH
    try {
        const which = execSync("which ctx", { encoding: "utf-8" }).trim();
        if (which)
            return which;
    }
    catch {
        // not on path
    }
    throw new Error("ctx binary not found. Set CTX_BIN env var or build with: cargo build --release");
}
const CTX_BIN = findCtxBinary();
// ── Auto-init: ensure project is initialized ────────────────────────
function ensureInitialized(projectPath) {
    try {
        execSync(`"${CTX_BIN}" -p "${projectPath}" status --json`, {
            encoding: "utf-8",
            timeout: 20_000,
            env: { ...process.env, NO_COLOR: "1" },
            maxBuffer: 10 * 1024 * 1024,
        });
        return;
    }
    catch {
        execSync(`"${CTX_BIN}" -p "${projectPath}" init`, {
            encoding: "utf-8",
            timeout: 60_000,
            env: { ...process.env, NO_COLOR: "1" },
            maxBuffer: 10 * 1024 * 1024,
        });
    }
}
function runCtx(args, projectPath, skipAutoInit = false) {
    // Auto-initialize if needed (skip for init command itself)
    if (!skipAutoInit) {
        try {
            ensureInitialized(projectPath);
        }
        catch {
            // init failed, continue anyway — the actual command will show the error
        }
    }
    const cmd = `"${CTX_BIN}" -p "${projectPath}" ${args}`;
    try {
        const output = execSync(cmd, {
            encoding: "utf-8",
            timeout: 30_000,
            env: { ...process.env, NO_COLOR: "1" },
            maxBuffer: 10 * 1024 * 1024,
        });
        // Strip ANSI escape codes
        const clean = output.replace(/\x1B\[[0-9;]*[a-zA-Z]/g, "").trim();
        return { success: true, output: clean };
    }
    catch (err) {
        const message = err instanceof Error ? err.message : String(err);
        return { success: false, output: `Error: ${message}` };
    }
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
server.tool("ctx_init", "Initialize ctx in a project directory. Creates a project-specific database in the global ctx store, scans all files, extracts symbols (functions, classes, structs), maps dependencies, and analyzes git history for decisions.", ProjectPathSchema.shape, async ({ project_path }) => {
    const { output } = runCtx("init", project_path, true);
    return { content: [{ type: "text", text: output }] };
});
// ── Tool: ctx_status ────────────────────────────────────────────────
server.tool("ctx_status", "Get project dashboard: total files, lines of code, symbols, dependencies, decisions, knowledge notes, and language breakdown.", ProjectPathSchema.shape, async ({ project_path }) => {
    const { output } = runCtx("status", project_path);
    return { content: [{ type: "text", text: output }] };
});
// ── Tool: ctx_map ───────────────────────────────────────────────────
server.tool("ctx_map", "Display a structured codebase map showing the directory tree with file counts, line counts, and language breakdown per directory. Ideal for understanding project structure at a glance.", ProjectPathSchema.shape, async ({ project_path }) => {
    const { output } = runCtx("map", project_path);
    return { content: [{ type: "text", text: output }] };
});
// ── Tool: ctx_scan ──────────────────────────────────────────────────
server.tool("ctx_scan", "Re-scan the project incrementally. Only analyzes files whose content hash has changed. Updates symbols, dependencies, and the full-text search index.", ProjectPathSchema.shape, async ({ project_path }) => {
    const { output } = runCtx("scan", project_path);
    return { content: [{ type: "text", text: output }] };
});
// ── Tool: ctx_query ─────────────────────────────────────────────────
server.tool("ctx_query", "Full-text search across all symbols (functions, classes, structs, enums, etc.) using FTS5. Returns matching symbols with their full signatures and file locations. Supports partial matching.", {
    ...ProjectPathSchema.shape,
    query: z
        .string()
        .describe("Search query — supports partial matches (e.g. 'parse', 'Database', 'init')"),
}, async ({ project_path, query }) => {
    const { output } = runCtx(`query "${query}"`, project_path);
    return { content: [{ type: "text", text: output }] };
});
// ── Tool: ctx_blast_radius ──────────────────────────────────────────
server.tool("ctx_blast_radius", "Analyze the blast radius of changing a specific file. Shows: what the file imports, what files depend on it, and the full transitive impact graph. Includes a risk assessment (low/medium/high/critical).", {
    ...ProjectPathSchema.shape,
    file_path: z
        .string()
        .describe("Relative path to the file (e.g. 'src/db/mod.rs')"),
}, async ({ project_path, file_path }) => {
    const { output } = runCtx(`blast-radius "${file_path}"`, project_path);
    return { content: [{ type: "text", text: output }] };
});
// ── Tool: ctx_decisions ─────────────────────────────────────────────
server.tool("ctx_decisions", "List all recorded architectural decisions. Includes decisions auto-extracted from conventional commits (feat/fix/refactor/breaking) and manually added entries.", ProjectPathSchema.shape, async ({ project_path }) => {
    const { output } = runCtx("decisions", project_path);
    return { content: [{ type: "text", text: output }] };
});
// ── Tool: ctx_learn ─────────────────────────────────────────────────
server.tool("ctx_learn", "Store a knowledge note about the project. Use this to record architectural insights, gotchas, design rationale, or any context that would help future development. Optionally link to a specific file.", {
    ...ProjectPathSchema.shape,
    note: z.string().describe("Knowledge note to record"),
    file: z
        .string()
        .optional()
        .describe("Optional: related file path for context"),
}, async ({ project_path, note, file }) => {
    const fileArg = file ? `--file "${file}"` : "";
    const { output } = runCtx(`learn "${note}" ${fileArg}`, project_path);
    return { content: [{ type: "text", text: output }] };
});
// ── Tool: ctx_warnings ──────────────────────────────────────────────
server.tool("ctx_warnings", "Show codebase health warnings: fragile files (high churn + many dependents), large files (>500 lines), and potentially dead code (no commits, no dependents). Helps prioritize refactoring.", ProjectPathSchema.shape, async ({ project_path }) => {
    const { output } = runCtx("warnings", project_path);
    return { content: [{ type: "text", text: output }] };
});
// ── Start server ────────────────────────────────────────────────────
async function main() {
    const transport = new StdioServerTransport();
    await server.connect(transport);
}
main().catch((err) => {
    console.error("ctx MCP server fatal error:", err.message);
    process.exit(1);
});
