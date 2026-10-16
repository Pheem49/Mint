import { createInterface } from 'node:readline'
import { MockDevice } from './mock-device.mjs'

const device = new MockDevice()
const deviceId = { type: 'string', const: 'mock-device', description: 'ID of this simulated motor and camera' }
const inputSchema = properties => ({ type: 'object', properties: { deviceId, ...properties }, required: ['deviceId', ...('rpm' in properties ? ['rpm'] : [])], additionalProperties: false })
const outputSchema = { type: 'object', properties: {
  deviceId: { type: 'string' }, operationId: { type: 'string' }, state: { enum: ['accepted', 'running', 'completed', 'failed'] },
  target: { type: 'object' }, actual: { type: 'object' }, observationSeq: { type: 'integer', minimum: 0 }, error: { type: 'string' },
}, required: ['deviceId', 'operationId', 'state', 'target', 'actual', 'observationSeq'], additionalProperties: false }
const commandMeta = { 'mint/device': { version: 1, statusTool: 'get_status' } }
const tools = [
  { name: 'get_status', description: 'Read a fresh device observation, optionally for an operationId; RPM readings are simulated.', inputSchema: inputSchema({ operationId: { type: 'string', minLength: 1 } }), outputSchema, annotations: { readOnlyHint: true } },
  { name: 'set_speed', description: 'Request a simulated motor speed, in integer RPM (0–3000). A receipt is not completion; use get_status to confirm.', inputSchema: inputSchema({ rpm: { type: 'integer', minimum: 0, maximum: 3000, description: 'Target speed in RPM' } }), outputSchema, _meta: commandMeta },
  { name: 'stop', description: 'Explicitly stop the simulated motor. Supersedes its running operation; verify zero RPM with get_status.', inputSchema: inputSchema({}), outputSchema, _meta: commandMeta },
  { name: 'capture_photo', description: 'Capture a generated PNG sample from the simulated camera; this is not a real photograph.', inputSchema: inputSchema({}), outputSchema, _meta: commandMeta },
]

function invoke(name, args) {
  const tool = tools.find(tool => tool.name === name)
  if (!tool) throw new Error('Unknown tool')
  if (!args || typeof args !== 'object' || Array.isArray(args) || args.deviceId !== 'mock-device') throw new Error('deviceId must be mock-device')
  if (Object.keys(args).some(key => !(key in tool.inputSchema.properties))) throw new Error('Unknown argument')
  if (name === 'set_speed' && (!Number.isInteger(args.rpm) || args.rpm < 0 || args.rpm > 3000)) throw new Error('rpm must be an integer from 0 to 3000')
  if ('operationId' in args && (typeof args.operationId !== 'string' || !args.operationId)) throw new Error('Invalid operationId')
  let state; let image
  if (name === 'get_status') state = device.getStatus(args.operationId)
  else if (name === 'set_speed') state = device.setSpeed(args.rpm)
  else if (name === 'stop') state = device.stop()
  else ({ state, image } = device.capturePhoto())
  return { content: [{ type: 'text', text: JSON.stringify(state) }, ...(image ? [{ type: 'image', mimeType: 'image/png', data: image }] : [])], structuredContent: state }
}

createInterface({ input: process.stdin }).on('line', line => {
  let request
  try { request = JSON.parse(line) } catch { process.stderr.write('Invalid MCP JSON\n'); return }
  // Cancel means stop waiting, never an implicit hardware stop command.
  if (request.id === undefined) return
  let result
  if (request.method === 'initialize') result = { protocolVersion: '2025-06-18', capabilities: { tools: {} }, serverInfo: { name: 'mint-device-mock', version: '1.0.0' } }
  else if (request.method === 'ping') result = {}
  else if (request.method === 'tools/list') result = { tools }
  else if (request.method === 'tools/call') {
    try { result = invoke(request.params?.name, request.params?.arguments) }
    catch (error) { result = { isError: true, content: [{ type: 'text', text: error.message }] } }
  } else { process.stdout.write(JSON.stringify({ jsonrpc: '2.0', id: request.id, error: { code: -32601, message: 'Unknown method' } }) + '\n'); return }
  process.stdout.write(JSON.stringify({ jsonrpc: '2.0', id: request.id, result }) + '\n')
})
