import type { AgentProgress, ChatResponse, DocumentAttachment } from '../types'
import { reduceLiveAgentProgressBatch } from '../agentProgress'

export interface ImageAttachment { dataUri: string; name: string; previewDataUri?: string; objectUrl?: string }
export interface VideoAttachment { dataUri: string; name: string }

export interface ConversationState {
  workspacePath: string
  message: string
  imageAttachments: ImageAttachment[]
  videoAttachments: VideoAttachment[]
  documentAttachment: DocumentAttachment | null
  sending: boolean
  sendingMessage: string
  sendingImageCount: number
  sendingVideoCount: number
  streamedReply: string
  streamedResponse: ChatResponse | null
  agentProgress: AgentProgress[]
  pendingApproval: any | null
}

export type ConversationEvent =
  | { type: 'compose'; message: string }
  | { type: 'workspace_selected'; path: string }
  | { type: 'image_attached'; image: ImageAttachment }
  | { type: 'image_removed'; index: number }
  | { type: 'video_attached'; video: VideoAttachment }
  | { type: 'video_removed'; index: number }
  | { type: 'document_attached'; document: DocumentAttachment | null }
  | { type: 'composer_cleared' }
  | { type: 'run_started'; message: string; imageCount: number; videoCount: number }
  | { type: 'chunk_received'; chunk: string }
  | { type: 'response_received'; response: ChatResponse }
  | { type: 'progress_received'; progress: AgentProgress[] }
  | { type: 'progress_cleared' }
  | { type: 'stream_cleared' }
  | { type: 'approval_requested'; approval: unknown }
  | { type: 'approval_resolved' }
  | { type: 'run_finished' }
  | { type: 'run_cancelled' }
  | { type: 'session_switched'; draft: string }

export function initialConversationState(initial: Partial<ConversationState> = {}): ConversationState {
  return {
    workspacePath: '', message: '', imageAttachments: [], videoAttachments: [],
    documentAttachment: null, sending: false, sendingMessage: '', sendingImageCount: 0,
    sendingVideoCount: 0, streamedReply: '', streamedResponse: null, agentProgress: [],
    pendingApproval: null, ...initial,
  }
}

export function transitionConversation(state: ConversationState, event: ConversationEvent): ConversationState {
  switch (event.type) {
    case 'compose': return { ...state, message: event.message }
    case 'workspace_selected': return { ...state, workspacePath: event.path }
    case 'image_attached': return state.imageAttachments.some((item) => item.name === event.image.name && item.dataUri === event.image.dataUri)
      ? state : { ...state, imageAttachments: [...state.imageAttachments, event.image] }
    case 'image_removed': return { ...state, imageAttachments: state.imageAttachments.filter((_, index) => index !== event.index) }
    case 'video_attached': return state.videoAttachments.some((item) => item.name === event.video.name && item.dataUri === event.video.dataUri)
      ? state : { ...state, videoAttachments: [...state.videoAttachments, event.video] }
    case 'video_removed': return { ...state, videoAttachments: state.videoAttachments.filter((_, index) => index !== event.index) }
    case 'document_attached': return { ...state, documentAttachment: event.document }
    case 'composer_cleared': return { ...state, message: '', imageAttachments: [], videoAttachments: [], documentAttachment: null }
    case 'run_started': return { ...state, sending: true, sendingMessage: event.message,
      sendingImageCount: event.imageCount, sendingVideoCount: event.videoCount,
      streamedReply: '', streamedResponse: null, agentProgress: [] }
    case 'chunk_received': return { ...state, streamedReply: state.streamedReply + event.chunk }
    case 'response_received': return { ...state, streamedResponse: event.response }
    case 'progress_received': return { ...state, agentProgress: reduceLiveAgentProgressBatch(state.agentProgress, event.progress) }
    case 'progress_cleared': return { ...state, agentProgress: [] }
    case 'stream_cleared': return { ...state, streamedReply: '', streamedResponse: null }
    case 'approval_requested': return { ...state, pendingApproval: event.approval }
    case 'approval_resolved': return { ...state, pendingApproval: null }
    case 'run_finished': return { ...state, sending: false, sendingMessage: '', sendingImageCount: 0, sendingVideoCount: 0 }
    case 'run_cancelled': return { ...state, sending: false, sendingMessage: '', sendingImageCount: 0,
      sendingVideoCount: 0, streamedReply: '', streamedResponse: null, agentProgress: [], pendingApproval: null }
    case 'session_switched': return { ...state, message: event.draft, imageAttachments: [], videoAttachments: [],
      documentAttachment: null, sending: false, sendingMessage: '', sendingImageCount: 0,
      sendingVideoCount: 0, streamedReply: '', streamedResponse: null, agentProgress: [], pendingApproval: null }
  }
}
