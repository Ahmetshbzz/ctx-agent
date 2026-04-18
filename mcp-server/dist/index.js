#!/usr/bin/env node
import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import { registerTools } from "./tools/register-tools.js";
const server = new McpServer({
    name: "ctx",
    version: "1.0.0",
});
registerTools(server);
async function main() {
    const transport = new StdioServerTransport();
    await server.connect(transport);
}
main().catch((err) => {
    console.error("ctx MCP server fatal error:", err.message);
    process.exit(1);
});
