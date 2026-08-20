// Stub for the desktop-only n8n/webhook workflow builder — no web
// equivalent, and the 'workflows' view is unreachable on web anyway
// (DashboardSidebar hides the Workflow tab via `hasWorkflowsTab`).
// MintDashboard.tsx imports this via the '@' alias, which resolves to the
// real src/components/WorkflowBuilderPanel.tsx on desktop.
export default function WorkflowBuilderPanel(_props: Record<string, unknown>) {
  return null
}
