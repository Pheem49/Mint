# Mint Device MCP example

This stdio MCP server simulates one motor and camera. It requires Node.js 18+ and has no npm dependencies. It never accesses real hardware. `mock-device.mjs` is the adapter seam; `server.mjs` owns MCP framing, tool descriptions, schemas, and results.

From the repository root, register its absolute path (replace `/path/to/Mint-CLI`):

```bash
mint mcp add mock-device node --args /path/to/Mint-CLI/examples/device-mcp/server.mjs
mint mcp allow mock-device '*'
```

In agent chat, ask: “Set mock-device to 100 RPM and confirm its observed speed”, “Stop mock-device and confirm zero RPM”, or “Capture a photo from mock-device”. The photo is a generated PNG sample. The allow command grants all tools on this example server; omit it to approve calls individually.

For the raw CLI, use:

```bash
mint mcp call mock-device set_speed --arguments '{"deviceId":"mock-device","rpm":100}'
```

The raw call returns the MCP receipt after schema validation. Automatic waiting and `DeviceState` events apply to agent chat. Read `get_status` with the receipt's `operationId` when using raw calls; repeated separate CLI processes start fresh mock servers and cannot preserve the simulated operation. An agent chat keeps the MCP session alive.

Run the example's protocol tests with `npm run test:device`. See [the Device v1 contract](contract.md) before replacing the mock adapter.
