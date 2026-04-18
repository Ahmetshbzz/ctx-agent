import { execFileSync } from "node:child_process";
import { runCtxArgv } from "./ctx-cli.js";

export interface GuardReport {
    mode: "paranoid" | "off";
    touchedFiles: string[];
    sensitiveFiles: string[];
    requiredControls: string[];
    missingControls: string[];
    status: "pass" | "block";
    risk: "low" | "medium" | "high" | "critical";
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
    const path = filePath.toLowerCase();
    return /(auth|session|token|jwt|crypto|cipher|tls|oauth|password|secret|cookie|csrf|admin)/.test(
        path
    );
}

function hasRepoPattern(projectPath: string, pattern: string): boolean {
    const safePattern = pattern.trim();
    if (!safePattern) return false;
    const { success, output } = runCtxArgv(["grep", safePattern, "--max-results", "1", "--json"], projectPath);
    if (!success) return false;
    try {
        const parsed = JSON.parse(output) as { count?: number };
        return (parsed.count ?? 0) > 0;
    } catch {
        return false;
    }
}

export function buildGuardReport(projectPath: string): GuardReport {
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

    const missingControls = controls.filter((control) => !control.ok).map((control) => control.name);

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
