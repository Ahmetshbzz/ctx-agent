import { execFileSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { CTX_BIN } from "./paths.js";

export interface CtxResult {
    success: boolean;
    output: string;
}

export function ensureInitialized(projectPath: string): void {
    try {
        execFileSync(CTX_BIN, ["-p", projectPath, "status", "--json"], {
            encoding: "utf-8",
            timeout: 20_000,
            env: { ...process.env, NO_COLOR: "1" },
            maxBuffer: 10 * 1024 * 1024,
        });
        return;
    } catch {
        execFileSync(CTX_BIN, ["-p", projectPath, "init"], {
            encoding: "utf-8",
            timeout: 60_000,
            env: { ...process.env, NO_COLOR: "1" },
            maxBuffer: 10 * 1024 * 1024,
        });
    }
}

export function runCtxArgv(args: string[], projectPath: string, skipAutoInit = false): CtxResult {
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
        const clean = output.replace(/\x1B\[[0-9;]*[a-zA-Z]/g, "").trim();
        return { success: true, output: clean };
    } catch (err: unknown) {
        const message = err instanceof Error ? err.message : String(err);
        return { success: false, output: `Error: ${message}` };
    }
}

export function safeRead(filePath: string, maxChars = 10_000): string {
    if (!existsSync(filePath)) return "";
    try {
        const content = readFileSync(filePath, "utf-8");
        return content.slice(0, maxChars);
    } catch {
        return "";
    }
}

export function runTextSearch(projectPath: string, pattern: string, maxResults = 60): string {
    const safeMax = Math.min(Math.max(maxResults, 1), 200);
    const { output } = runCtxArgv(["grep", pattern, "--max-results", String(safeMax)], projectPath);
    return output || "No text matches found.";
}
