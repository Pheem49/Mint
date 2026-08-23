// Stub for the desktop-only Live2D companion character panel — web has no
// equivalent (no cubism4 runtime is bundled), so this renders nothing.
// MintDashboard.tsx imports `ModelPanel` via the '@' alias, which resolves
// to this file on web and to the real src/components/ModelPanel.tsx on
// desktop; the companion widget's own state (from '@/companionWidget')
// already reports `modelVisible: false` on web, so nothing ever tries to
// meaningfully interact with this stub.
export type ModelInteraction = 'head' | 'cheek' | 'left hand' | 'right hand' | 'body' | 'lower body'

export default function ModelPanel(_props: Record<string, unknown>) {
  return null
}
