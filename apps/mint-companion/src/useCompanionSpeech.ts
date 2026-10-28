import { useEffect, useRef, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { SpeechPlayer, type SpeechState } from '../../../src/renderer/shared/speech/player'
import type { SpeechReply, SpeechRequest } from '../../../src/renderer/shared/speech/types'
import { AutoSpeechTracker, mouthFromSamples } from './speech-policy'
import { terminal, type Turn } from './state'

type Input = { hostId: string; chatId: string; online: boolean; ready: boolean; turns: Turn[]; auto: boolean; muted: boolean; volume: number }
export function useCompanionSpeech(input: Input) {
  const [state, setState] = useState<SpeechState>('idle')
  const [notice, setNotice] = useState('')
  const mouth = useRef(0)
  const active = useRef(false)
  const current = useRef(input); current.current = input
  const owners = useRef(new Map<string, string>())
  const preference = useRef('')
  const tracker = useRef(new AutoSpeechTracker())
  const latestTurn = useRef(0)
  const player = useRef<SpeechPlayer | null>(null)
  if (!player.current) player.current = new SpeechPlayer(async (request: SpeechRequest) => {
    if (request.operation === 'prepare') { owners.current.set(request.owner, current.current.hostId); if (request.automatic) request = { ...request, routeOwner: preference.current } }
    const hostId = owners.current.get(request.owner) ?? current.current.hostId
    try { return await invoke<SpeechReply>('speech_request', { hostId, request }) }
    finally { if (request.operation === 'release') owners.current.delete(request.owner) }
  }, next => { active.current = next === 'speaking'; setState(next) }, setNotice, samples => { mouth.current = samples ? mouthFromSamples(samples, mouth.current) : 0 })

  useEffect(() => {
    void player.current!.stop(); tracker.current.reset(); latestTurn.current = 0; setNotice('')
    return () => { void player.current!.stop(); mouth.current = 0 }
  }, [input.hostId, input.chatId, input.online])
  useEffect(() => { player.current!.volume(input.volume); if (input.muted) void player.current!.stop() }, [input.muted, input.volume])
  useEffect(() => {
    if (!input.online || !input.auto || !input.hostId || !input.chatId) return
    const owner = crypto.randomUUID(); preference.current = owner
    const hostId = input.hostId; const chatId = input.chatId
    let disposed = false; let pending = false
    const reserve = async () => {
      if (pending) return
      pending = true
      try {
        const reply = await invoke<SpeechReply>('speech_request', { hostId, request: { operation: 'prefer', owner, chatId, enabled: true } })
        if (!disposed && reply.kind === 'lease' && !reply.active) setNotice('Another Companion owns automatic speech for this conversation.')
      } catch (reason) { if (!disposed) setNotice(String(reason)) }
      finally { pending = false }
    }
    void reserve(); const timer = setInterval(() => { void reserve() }, 3000)
    return () => { disposed = true; clearInterval(timer); if (preference.current === owner) preference.current = ''; void invoke('speech_request', { hostId, request: { operation: 'prefer', owner, chatId, enabled: false } }).catch(() => {}) }
  }, [input.hostId, input.chatId, input.online, input.auto])
  useEffect(() => {
    if (!input.online || !input.ready) return
    const turns = input.turns.filter(turn => turn.chatId === input.chatId)
    const latest = Math.max(0, ...turns.map(turn => turn.turnId))
    if (latestTurn.current && latest > latestTurn.current) void player.current!.stop()
    latestTurn.current = Math.max(latest, latestTurn.current)
    const id = tracker.current.observe(turns)
    if (id !== null && input.auto && !input.muted && !turns.some(turn => turn.turnId > id && !terminal(turn.status))) {
      setNotice('')
      void player.current!.play({ chatId: input.chatId, turnId: id, automatic: true, audioOnly: true })
    }
  }, [input.turns, input.online, input.ready, input.auto, input.muted, input.chatId])
  useEffect(() => () => { void player.current!.dispose() }, [])
  return {
    state, notice, mouth, active,
    unlock: () => { void player.current!.unlock().catch(reason => setNotice(String(reason))) },
    stop: () => { void player.current!.stop() },
    read: () => {
      const input = current.current
      if (!input.online || input.muted) return
      const latest = input.turns.filter(turn => turn.chatId === input.chatId && turn.status === 'completed' && turn.text.trim()).sort((a,b) => b.turnId-a.turnId)[0]
      if (!latest) { setNotice('No completed reply to read yet.'); return }
      setNotice('')
      void player.current!.play({ chatId: input.chatId, turnId: latest.turnId, audioOnly: true })
    },
  }
}
