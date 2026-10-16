import { DEFAULT_CONFIG } from '../shared/constants/config'

const noop = () => {}
export function chatPanelFixture() {
  return {
    conversation: {
      interactions: [], sending: false, sendingMessage: '', sendingImageCount: 0,
      streamedReply: '', streamedResponse: null, agentProgress: [], agentActivitySnapshots: {},
      thinkingExpanded: {}, message: '', imageAttachments: [], videoAttachments: [], documentName: '',
      pendingApproval: null, agentMode: false, status: null, chatId: 'A', recentWorkspacePaths: [],
      chatEnd: { current: null }, welcomeInteraction: null, settingsConfig: DEFAULT_CONFIG,
      conversationTitle: 'Session sources example',
    },
    actions: Object.fromEntries([
      'onSubmit', 'onSelectImage', 'onSelectVideo', 'onSelectDocument', 'onPasteImage', 'onSetMessage',
      'onSendVoiceMessage', 'onRemoveImage', 'onRemoveVideo', 'onRemoveDocument', 'onStartWebSearch',
      'onCaptureScreen', 'onSetAgentMode', 'onSetProvider', 'onApproval', 'onSetModel',
      'onCancelMessage', 'onClearMessages', 'onSetGeminiLiveVoice', 'onThinkingExpandedChange',
      'onToggleMobileSidebar',
    ].map(name => [name, noop])),
    tools: { modsPanelTarget: null, onOpenMods: noop, onToggle: noop, open: false, backgroundTaskCount: 0 },
  }
}
