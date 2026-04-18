import { existsSync, readdirSync } from "node:fs";
import { join } from "node:path";
import { runCtxArgv, safeRead } from "./ctx-cli.js";

export interface ProjectOverview {
    bullets: string[];
    sources: string[];
    note: string;
}

function compactParagraph(text: string, maxLen = 260): string {
    const cleaned = text
        .replace(/```[\s\S]*?```/g, " ")
        .replace(/<[^>]+>/g, " ")
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
        .replace(/<[^>]+>/g, " ")
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
        .map((paragraph) => compactParagraph(stripMarkdownInline(paragraph), 260))
        .filter((paragraph) => paragraph.length >= 60 && /[a-zA-Z]/.test(paragraph));
    return paragraphs[0] ? compactParagraph(paragraphs[0], 220) : "";
}

function detectTopModules(projectPath: string): string[] {
    try {
        return readdirSync(projectPath, { withFileTypes: true })
            .filter((dirent) => dirent.isDirectory())
            .map((dirent) => dirent.name)
            .filter((name) => !name.startsWith("."))
            .slice(0, 8);
    } catch {
        return [];
    }
}

function dedupeSourceCandidates(paths: string[]): string[] {
    const seen = new Set<string>();
    const deduped: string[] = [];
    for (const path of paths) {
        const normalized = path.toLowerCase();
        if (seen.has(normalized)) continue;
        seen.add(normalized);
        deduped.push(path);
    }
    return deduped;
}

export function buildProjectOverview(projectPath: string): ProjectOverview {
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
    const existingSources = dedupeSourceCandidates(
        sourceCandidates.filter((path) => existsSync(join(projectPath, path)))
    );
    const docsCombined = existingSources
        .map((path) => safeRead(join(projectPath, path)))
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
        `3) Main modules: ${hasApps ? "multi-app workspace under \`apps/\`" : "monolithic project layout"}${hasCore ? " with a backend core service" : ""}${hasPanel ? ", admin panel" : ""}${hasClient ? ", and client frontend" : ""}.`,
        `4) Backend responsibility: central API routing, service orchestration, and business logic${hasTenant ? " with tenant-aware isolation" : ""}.`,
        "5) Critical runtime flow: request handling across auth, admin operations, and domain-specific endpoints via backend route registration.",
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

export function ensureOverviewNoteIfNeeded(projectPath: string): "saved" | "skipped" | "failed" {
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
