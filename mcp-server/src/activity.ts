import { createHash } from "node:crypto";
import { appendFileSync, existsSync, mkdirSync, readFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { homedir } from "node:os";

export interface ActivityEntry {
    ts: string;
    tool: string;
    summary: string;
}

interface ActivityEmitState {
    emitted: boolean;
}

const activityEmitState = new Map<string, ActivityEmitState>();

function activityFilePath(projectPath: string): string {
    const canonical = resolve(projectPath);
    const key = createHash("sha256").update(canonical).digest("hex");
    const dir = join(homedir(), ".ctx-agent", "activity");
    mkdirSync(dir, { recursive: true });
    return join(dir, `${key}.jsonl`);
}

function recordActivity(projectPath: string, tool: string, summary: string): void {
    try {
        const entry: ActivityEntry = {
            ts: new Date().toISOString(),
            tool,
            summary,
        };
        appendFileSync(activityFilePath(projectPath), `${JSON.stringify(entry)}\n`, "utf-8");
    } catch {
        // best-effort logging
    }
}

function recentActivity(projectPath: string, count = 5): ActivityEntry[] {
    try {
        const file = activityFilePath(projectPath);
        if (!existsSync(file)) return [];
        const lines = readFileSync(file, "utf-8")
            .split(/\r?\n/)
            .map((line) => line.trim())
            .filter(Boolean);
        const parsed: ActivityEntry[] = lines
            .map((line) => {
                try {
                    return JSON.parse(line) as ActivityEntry;
                } catch {
                    return null;
                }
            })
            .filter((entry): entry is ActivityEntry => Boolean(entry));
        return parsed.slice(-count);
    } catch {
        return [];
    }
}

export function withRecentActivity(
    projectPath: string,
    baseText: string,
    tool: string,
    summary: string
): string {
    recordActivity(projectPath, tool, summary);
    const recents = recentActivity(projectPath, 5);
    if (recents.length === 0) return baseText;

    const projectKey = resolve(projectPath);
    const prev = activityEmitState.get(projectKey);
    const shouldEmit = !prev || !prev.emitted;
    activityEmitState.set(projectKey, { emitted: true });
    if (!shouldEmit) return baseText;

    const lines = [
        "",
        "",
        "Recent agent activity (last 5):",
        ...recents.map((entry) => `- ${entry.ts} | ${entry.tool} | ${entry.summary}`),
    ];
    return `${baseText}${lines.join("\n")}`;
}
