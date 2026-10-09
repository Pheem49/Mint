# Mint Device v1 contract

## Discovery and arguments

Every MCP tool must publish a usable `inputSchema` declaring an object. Mint loads all `tools/list` pages before selecting a tool and validates arguments locally for raw CLI/API and agent calls. Unknown names, missing/unusable schemas, and invalid arguments are rejected before `tools/call`. Values are never coerced and defaults are never inserted. Extra properties are accepted only as allowed by the schema. JSON Schema local references work; unresolved network and file references are blocked.

The catalog is scoped to the live server session and its connection configuration. Reconnects, configuration changes, `notifications/tools/list_changed`, and a 60-second TTL invalidate it. Discovery has a 30-second deadline and bounds of 100 pages, 1,000 tools, and 256 KiB per definition. Oversized or partial schemas are rejected. Agent preparation supplies the full description/schema to the next model request, preserves it after context compaction, and requests a new command proposal. Reading definitions grants no command permission. Preparation events do not count as tool executions or argument retries.

All discovery pages, validation, command dispatch, and subsequent status polls remain bound to that session and catalog generation. Mint never reconnects within an invocation. Session loss before dispatch rejects the command; loss after dispatch stops verification with completion unconfirmed, preserving the receipt and latest observation. A replacement connector cannot verify the old operation. A later explicit invocation can establish a new session and read its schemas.

## Opting into verification

A command opts in by publishing this metadata in its tool definition:

```json
{"_meta":{"mint/device":{"version":1,"statusTool":"get_status"}}}
```

Names such as `set_speed` or `stop` alone do not activate the contract. Command and status tools must both provide usable `inputSchema` and `outputSchema`. `statusTool` is a read-only tool accepting `deviceId` and the opaque `operationId` returned by the command. It must accept a string operation identifier, including UUIDs. It cannot itself carry command metadata. Mint validates the command, verifies the status definition, and obtains command and status-read permissions before dispatching any command. A single-call status grant is held only in that command's cloned configuration for its poll sequence. The approval describes status access scoped to the operation ID returned by this command; the actual ID is not known before dispatch.

Both results expose `structuredContent` with:

```json
{
  "deviceId":"mock-device",
  "operationId":"opaque-id",
  "state":"accepted",
  "target":{"rpm":100},
  "actual":{"rpm":0},
  "observationSeq":1
}
```

`deviceId` and `operationId` are nonempty strings; `target` and `actual` are objects. `observationSeq` is a nonnegative integer increasing with each fresh physical observation. `state` is `accepted`, `running`, `completed`, or `failed`; `error` may describe a failure. Keep the target stable throughout an operation. Attach images using standard MCP image content alongside the structured receipt; Mint preserves them through verification.

- `accepted`: the adapter received the command. This does not prove physical action or completion.
- `running`: the operation is active. `actual` comes from a current device reading.
- `completed`: the adapter has verified the target using a fresh physical observation. A successful write to USB/Serial or a published MQTT message is insufficient. Account for tolerances, encoder position, motor speed, or camera capture in the adapter's completion rule.
- `failed`: the operation has failed. Preserve its identity, target, latest observation, and diagnostic.

Mint polls approximately once per second. It accepts completion only for the same device, operation ID, and target, with a sequence newer than both the command receipt and the previous accepted observation. Repeated/stale samples cannot confirm completion. Identity/target mismatch, malformed observations, or an `isError` response stop verification without reporting success. A changed status definition also stops verification. The adapter remains responsible for the truth of sensor readings and the meaning of `completed`; MCP cannot independently inspect the hardware.

The existing server `timeoutSecs` is one overall deadline for command dispatch and verification (default 300 seconds). Each status request waits at most 10 seconds or the remaining deadline. Mint emits `accepted`, `running`, `verified`, `failed`, and `unconfirmed` DeviceState events. Only a fresh completed observation produces successful ToolEnd. A failure to verify preserves the receipt and attachments and explains that the command must not be repeated automatically. The agent blocks an identical Device command with unconfirmed completion for the rest of that user turn (argument key order does not change its identity). Status reads and a distinct explicit stop remain available; a new user instruction starts a new turn.

## Adapter responsibilities

Keep transport details in the adapter: USB/Serial framing, MQTT topics, reconnects, authentication, command queueing, units, numeric limits, sensor sampling, and completion rules. Expose only real capabilities, with clear descriptions and bounded input schemas. Use a unique operation ID per command; never silently turn a status request into a command. The mock rejects a second speed command while busy and an explicit `stop` supersedes its current operation with a failed state.

Chat cancellation stops waiting and polling and requests MCP cancellation where possible. It does not physically stop the device. Stopping hardware requires an explicit, separately authorized stop command, followed by fresh status verification. Connector/device restarts do not resume Mint's active jobs; if an operation becomes unknown, report an error instead of manufacturing completion. Completed activity and attachments remain in chat history.
