import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { z } from "zod";
import { withRecentActivity } from "../activity.js";
import { runCtxArgv, runTextSearch } from "../ctx-cli.js";
import { buildGuardReport } from "../guard.js";
import { buildProjectOverview, ensureOverviewNoteIfNeeded } from "../overview.js";
import { ProjectPathSchema } from "../schemas.js";

export function registerTools(server: McpServer): void {
    server.tool(
        "ctx_init",
        "Initialize ctx in a project directory. Creates a project-specific database in the global ctx store, scans all files, extracts symbols (functions, classes, structs), maps dependencies, and analyzes git history for decisions.",
        ProjectPathSchema.shape,
        async ({ project_path }) => {
            const { output } = runCtxArgv(["init"], project_path, true);
            const text = withRecentActivity(project_path, output, "ctx_init", "initialize or re-scan project index");
            return { content: [{ type: "text" as const, text }] };
        }
    );

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
                guard.sensitiveFiles.slice(0, 10).forEach((filePath) => guardLines.push(`- ${filePath}`));
            }
            if (guard.status === "block") {
                guardLines.push("Missing controls:");
                guard.missingControls.forEach((control) => guardLines.push(`- ${control}`));
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
            const text = withRecentActivity(
                project_path,
                `${output}${suffix}`,
                "ctx_status",
                "fetch project status dashboard"
            );
            return { content: [{ type: "text" as const, text }] };
        }
    );

    server.tool(
        "ctx_map",
        "Display a structured codebase map showing the directory tree with file counts, line counts, and language breakdown per directory. Ideal for understanding project structure at a glance.",
        ProjectPathSchema.shape,
        async ({ project_path }) => {
            const { output } = runCtxArgv(["map"], project_path);
            const text = withRecentActivity(project_path, output, "ctx_map", "render codebase map");
            return { content: [{ type: "text" as const, text }] };
        }
    );

    server.tool(
        "ctx_scan",
        "Re-scan the project incrementally. Only analyzes files whose content hash has changed. Updates symbols, dependencies, and the full-text search index.",
        ProjectPathSchema.shape,
        async ({ project_path }) => {
            const { output } = runCtxArgv(["scan"], project_path);
            const text = withRecentActivity(project_path, output, "ctx_scan", "incremental re-scan");
            return { content: [{ type: "text" as const, text }] };
        }
    );

    server.tool(
        "ctx_query",
        "Full-text search across all symbols (functions, classes, structs, enums, etc.) using FTS5. Returns matching symbols with their full signatures and file locations. Supports partial matching.",
        {
            ...ProjectPathSchema.shape,
            query: z
                .string()
                .describe("Search query — supports partial matches (e.g. 'parse', 'Database', 'init')"),
        },
        async ({ project_path, query }) => {
            const { output } = runCtxArgv(["query", query], project_path);
            if (!output.includes("No results found.")) {
                const text = withRecentActivity(project_path, output, "ctx_query", `symbol query: ${query}`);
                return { content: [{ type: "text" as const, text }] };
            }
            const fallback = runTextSearch(project_path, query, 60);
            const merged = [
                output,
                "",
                "Text search fallback (ctx-agent built-in grep):",
                fallback,
            ].join("\n");
            const text = withRecentActivity(
                project_path,
                merged,
                "ctx_query",
                `symbol query with fallback: ${query}`
            );
            return { content: [{ type: "text" as const, text }] };
        }
    );

    server.tool(
        "ctx_blast_radius",
        "Analyze the blast radius of changing a specific file. Shows: what the file imports, what files depend on it, and the full transitive impact graph. Includes a risk assessment (low/medium/high/critical).",
        {
            ...ProjectPathSchema.shape,
            file_path: z.string().describe("Relative path to the file (e.g. 'src/db/mod.rs')"),
        },
        async ({ project_path, file_path }) => {
            const { output } = runCtxArgv(["blast-radius", file_path], project_path);
            const text = withRecentActivity(project_path, output, "ctx_blast_radius", `blast radius for ${file_path}`);
            return { content: [{ type: "text" as const, text }] };
        }
    );

    server.tool(
        "ctx_decisions",
        "List all recorded architectural decisions. Includes decisions auto-extracted from conventional commits (feat/fix/refactor/breaking) and manually added entries.",
        ProjectPathSchema.shape,
        async ({ project_path }) => {
            const { output } = runCtxArgv(["decisions"], project_path);
            const text = withRecentActivity(project_path, output, "ctx_decisions", "list decision history");
            return { content: [{ type: "text" as const, text }] };
        }
    );

    server.tool(
        "ctx_learn",
        "Store a knowledge note about the project. Use this to record architectural insights, gotchas, design rationale, or any context that would help future development. Optionally link to a specific file.",
        {
            ...ProjectPathSchema.shape,
            note: z.string().describe("Knowledge note to record"),
            file: z.string().optional().describe("Optional: related file path for context"),
        },
        async ({ project_path, note, file }) => {
            const args = file ? ["learn", note, "--file", file] : ["learn", note];
            const { output } = runCtxArgv(args, project_path);
            const text = withRecentActivity(
                project_path,
                output,
                "ctx_learn",
                file ? `store knowledge note for ${file}` : "store knowledge note"
            );
            return { content: [{ type: "text" as const, text }] };
        }
    );

    server.tool(
        "ctx_warnings",
        "Show codebase health warnings: fragile files (high churn + many dependents), large files (>500 lines), and potentially dead code (no commits, no dependents). Helps prioritize refactoring.",
        ProjectPathSchema.shape,
        async ({ project_path }) => {
            const { output } = runCtxArgv(["warnings"], project_path);
            const text = withRecentActivity(project_path, output, "ctx_warnings", "list codebase warnings");
            return { content: [{ type: "text" as const, text }] };
        }
    );

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
            const finalText = withRecentActivity(
                project_path,
                text,
                "ctx_overview",
                `generate project overview (save_note=${save_note !== false})`
            );
            return { content: [{ type: "text" as const, text: finalText }] };
        }
    );

    server.tool(
        "ctx_watch_status",
        "Show background watch health for a project, including whether the watcher is running, its PID, and the last observed scan/event timestamps.",
        ProjectPathSchema.shape,
        async ({ project_path }) => {
            const { output } = runCtxArgv(["watch-status", "--json"], project_path, true);
            const text = withRecentActivity(project_path, output, "ctx_watch_status", "fetch watch health status");
            return { content: [{ type: "text" as const, text }] };
        }
    );

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
                guard.sensitiveFiles.forEach((filePath) => lines.push(`- ${filePath}`));
            }
            lines.push("Required controls:");
            guard.requiredControls.forEach((control) => lines.push(`- ${control}`));
            if (guard.missingControls.length > 0) {
                lines.push("Missing controls:");
                guard.missingControls.forEach((control) => lines.push(`- ${control}`));
            }
            const text = withRecentActivity(
                project_path,
                lines.join("\n"),
                "ctx_guard",
                "run paranoid security guard checks"
            );
            return { content: [{ type: "text" as const, text }] };
        }
    );

    server.tool(
        "ctx_grep",
        "Fast text search across the repository using ctx-agent built-in grep (ripgrep-style in Rust). Useful when a symbol query misses strings, routes, handlers, or comments.",
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
            const text = withRecentActivity(project_path, output, "ctx_grep", `text search: ${pattern}`);
            return { content: [{ type: "text" as const, text }] };
        }
    );
}
