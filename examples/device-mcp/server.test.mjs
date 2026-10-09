import { test } from 'node:test'
import assert from 'node:assert/strict'
import { spawn } from 'node:child_process'
import { createInterface } from 'node:readline'
import { fileURLToPath } from 'node:url'

function connect(t) {
  const child = spawn(process.execPath, [fileURLToPath(new URL('./server.mjs', import.meta.url))], { stdio: ['pipe', 'pipe', 'pipe'] })
  const pending = new Map(); let id = 0
  createInterface({ input: child.stdout }).on('line', line => {
    const message = JSON.parse(line); const waiter = pending.get(message.id)
    if (waiter) { pending.delete(message.id); message.error ? waiter.reject(new Error(message.error.message)) : waiter.resolve(message.result) }
  })
  child.on('exit', () => { for (const waiter of pending.values()) waiter.reject(new Error('Device MCP exited')); pending.clear() })
  t.after(() => child.kill())
  const rpc = (method, params = {}) => new Promise((resolve, reject) => {
    pending.set(++id, { resolve, reject }); child.stdin.write(JSON.stringify({ jsonrpc: '2.0', id, method, params }) + '\n')
  })
  rpc.notify = (method,params) => child.stdin.write(JSON.stringify({jsonrpc:'2.0',method,params})+'\n')
  return rpc
}

test('publishes only the supported capabilities with units and strict schemas', async t => {
  const rpc = connect(t); await rpc('initialize')
  const result = await rpc('tools/list')
  assert.deepEqual(result.tools.map(t => t.name).sort(), ['capture_photo', 'get_status', 'set_speed', 'stop'])
  const speed = result.tools.find(t => t.name === 'set_speed')
  assert.equal(speed.inputSchema.properties.rpm.maximum, 3000)
  assert.match(speed.description, /RPM/)
})

test('acknowledges speed separately from a fresh completed observation', async t => {
  const rpc = connect(t); await rpc('initialize')
  const receipt = await rpc('tools/call', { name: 'set_speed', arguments: { deviceId: 'mock-device', rpm: 100 } })
  assert.equal(receipt.structuredContent.state, 'accepted')
  assert.equal(receipt.structuredContent.actual.rpm, 0)
  const operationId = receipt.structuredContent.operationId
  let status
  const until = Date.now() + 5000
  do {
    await new Promise(resolve => setTimeout(resolve, 100))
    status = await rpc('tools/call', { name: 'get_status', arguments: { deviceId: 'mock-device', operationId } })
  } while (status.structuredContent.state !== 'completed' && Date.now() < until)
  assert.equal(status.structuredContent.state, 'completed')
  assert.equal(status.structuredContent.actual.rpm, 100)
  assert.ok(status.structuredContent.observationSeq > receipt.structuredContent.observationSeq)
})

test('stop supersedes a running command', async t => {
  const rpc = connect(t); await rpc('initialize')
  const call = await rpc('tools/call', { name: 'set_speed', arguments: { deviceId: 'mock-device', rpm: 1000 } })
  const stopped = await rpc('tools/call', { name: 'stop', arguments: { deviceId: 'mock-device' } })
  const old = await rpc('tools/call', { name: 'get_status', arguments: { deviceId: 'mock-device', operationId: call.structuredContent.operationId } })
  assert.equal(old.structuredContent.state, 'failed')
  const current = await rpc('tools/call', { name: 'get_status', arguments: { deviceId: 'mock-device', operationId: stopped.structuredContent.operationId } })
  assert.equal(current.structuredContent.actual.rpm, 0)
  assert.equal(current.structuredContent.state, 'completed')
})

test('rejects invalid commands and returns an image through capture_photo', async t => {
  const rpc = connect(t); await rpc('initialize')
  const bad = await rpc('tools/call', { name: 'set_speed', arguments: { deviceId: 'mock-device', rpm: 'fast' } })
  assert.equal(bad.isError, true)
  const image = await rpc('tools/call', { name: 'capture_photo', arguments: { deviceId: 'mock-device' } })
  assert.equal(image.content.find(c => c.type === 'image').mimeType, 'image/png')
  assert.equal(image.structuredContent.state, 'completed')
})


test('cancel notification stops no physical operation', async t => {
  const rpc=connect(t);await rpc('initialize')
  const receipt=await rpc('tools/call',{name:'set_speed',arguments:{deviceId:'mock-device',rpm:1000}})
  rpc.notify('notifications/cancelled',{requestId:2,reason:'Stopped waiting'})
  await new Promise(resolve=>setTimeout(resolve,2100))
  const status=await rpc('tools/call',{name:'get_status',arguments:{deviceId:'mock-device',operationId:receipt.structuredContent.operationId}})
  assert.equal(status.structuredContent.state,'completed')
  assert.equal(status.structuredContent.actual.rpm,1000)
})
