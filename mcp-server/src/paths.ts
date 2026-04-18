import { execFileSync } from "node:child_process";
import { existsSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

function findCtxBinary(): string {
    if (process.env.CTX_BIN && existsSync(process.env.CTX_BIN)) {
        return process.env.CTX_BIN;
    }

    const projectRoot = resolve(__dirname, "../..");
    const release = resolve(projectRoot, "target/release/ctx");
    const debug = resolve(projectRoot, "target/debug/ctx");
    if (existsSync(release)) return release;
    if (existsSync(debug)) return debug;

    try {
        const which = execFileSync("which", ["ctx"], { encoding: "utf-8" }).trim();
        if (which) return which;
    } catch {
        // not on path
    }

    throw new Error(
        "ctx binary not found. Set CTX_BIN env var or build with: cargo build --release"
    );
}

export const CTX_BIN = findCtxBinary();
