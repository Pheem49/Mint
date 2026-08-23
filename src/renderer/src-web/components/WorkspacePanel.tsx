// Stub for the desktop-only local-workspace-folder panel — web has no
// local filesystem to browse, and the 'workspace' view is unreachable on
// web anyway (DashboardSidebar hides the Workspace tab via
// `showWorkspaceTab`). MintDashboard.tsx imports this via the '@' alias,
// which resolves to the real src/components/WorkspacePanel.tsx on desktop.
export default function WorkspacePanel(_props: Record<string, unknown>) {
  return null
}
