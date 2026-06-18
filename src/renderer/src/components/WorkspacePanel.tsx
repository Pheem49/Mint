import { useEffect, useState } from 'react'
import { getWorkspaceTree, type WorkspaceTreeEntry } from '../tauri'
import folderIcon from 'material-icon-theme/icons/folder.svg?url'
import folderOpenIcon from 'material-icon-theme/icons/folder-open.svg?url'
import folderComponentsIcon from 'material-icon-theme/icons/folder-components.svg?url'
import folderComponentsOpenIcon from 'material-icon-theme/icons/folder-components-open.svg?url'
import folderConfigIcon from 'material-icon-theme/icons/folder-config.svg?url'
import folderConfigOpenIcon from 'material-icon-theme/icons/folder-config-open.svg?url'
import folderDistIcon from 'material-icon-theme/icons/folder-dist.svg?url'
import folderDistOpenIcon from 'material-icon-theme/icons/folder-dist-open.svg?url'
import folderDocsIcon from 'material-icon-theme/icons/folder-docs.svg?url'
import folderDocsOpenIcon from 'material-icon-theme/icons/folder-docs-open.svg?url'
import folderGitIcon from 'material-icon-theme/icons/folder-git.svg?url'
import folderGitOpenIcon from 'material-icon-theme/icons/folder-git-open.svg?url'
import folderGithubIcon from 'material-icon-theme/icons/folder-github.svg?url'
import folderGithubOpenIcon from 'material-icon-theme/icons/folder-github-open.svg?url'
import folderImagesIcon from 'material-icon-theme/icons/folder-images.svg?url'
import folderImagesOpenIcon from 'material-icon-theme/icons/folder-images-open.svg?url'
import folderLogIcon from 'material-icon-theme/icons/folder-log.svg?url'
import folderLogOpenIcon from 'material-icon-theme/icons/folder-log-open.svg?url'
import folderNodeIcon from 'material-icon-theme/icons/folder-node.svg?url'
import folderNodeOpenIcon from 'material-icon-theme/icons/folder-node-open.svg?url'
import folderPublicIcon from 'material-icon-theme/icons/folder-public.svg?url'
import folderPublicOpenIcon from 'material-icon-theme/icons/folder-public-open.svg?url'
import folderRustIcon from 'material-icon-theme/icons/folder-rust.svg?url'
import folderRustOpenIcon from 'material-icon-theme/icons/folder-rust-open.svg?url'
import folderScriptsIcon from 'material-icon-theme/icons/folder-scripts.svg?url'
import folderScriptsOpenIcon from 'material-icon-theme/icons/folder-scripts-open.svg?url'
import folderSrcIcon from 'material-icon-theme/icons/folder-src.svg?url'
import folderSrcOpenIcon from 'material-icon-theme/icons/folder-src-open.svg?url'
import folderSrcTauriIcon from 'material-icon-theme/icons/folder-src-tauri.svg?url'
import folderSrcTauriOpenIcon from 'material-icon-theme/icons/folder-src-tauri-open.svg?url'
import folderTestIcon from 'material-icon-theme/icons/folder-test.svg?url'
import folderTestOpenIcon from 'material-icon-theme/icons/folder-test-open.svg?url'
import folderUiIcon from 'material-icon-theme/icons/folder-ui.svg?url'
import folderUiOpenIcon from 'material-icon-theme/icons/folder-ui-open.svg?url'
import folderUtilsIcon from 'material-icon-theme/icons/folder-utils.svg?url'
import folderUtilsOpenIcon from 'material-icon-theme/icons/folder-utils-open.svg?url'
import cssIcon from 'material-icon-theme/icons/css.svg?url'
import documentIcon from 'material-icon-theme/icons/document.svg?url'
import htmlIcon from 'material-icon-theme/icons/html.svg?url'
import imageIcon from 'material-icon-theme/icons/image.svg?url'
import javascriptIcon from 'material-icon-theme/icons/javascript.svg?url'
import jsonIcon from 'material-icon-theme/icons/json.svg?url'
import lockIcon from 'material-icon-theme/icons/lock.svg?url'
import logIcon from 'material-icon-theme/icons/log.svg?url'
import markdownIcon from 'material-icon-theme/icons/markdown.svg?url'
import npmIcon from 'material-icon-theme/icons/npm.svg?url'
import reactIcon from 'material-icon-theme/icons/react.svg?url'
import reactTsIcon from 'material-icon-theme/icons/react_ts.svg?url'
import rustIcon from 'material-icon-theme/icons/rust.svg?url'
import settingsIcon from 'material-icon-theme/icons/settings.svg?url'
import tauriIcon from 'material-icon-theme/icons/tauri.svg?url'
import tomlIcon from 'material-icon-theme/icons/toml.svg?url'
import typescriptIcon from 'material-icon-theme/icons/typescript.svg?url'
import viteIcon from 'material-icon-theme/icons/vite.svg?url'
import yamlIcon from 'material-icon-theme/icons/yaml.svg?url'

interface WorkspacePanelProps {
  agentMode: boolean
  sending: boolean
  workspacePath: string
  onEnableAgentMode: () => void
  onSetMessage: (message: string) => void
  onWorkspaceReady: (path: string) => void
}

const FILE_LABEL: Record<string, string> = {
  css: '#',
  html: '<>',
  js: 'JS',
  json: '{}',
  md: 'MD',
  rs: 'RS',
  ts: 'TS',
  tsx: 'TS',
}

const FOLDER_ICONS: Record<string, { closed: string; open: string }> = {
  '.cargo_home': { closed: folderRustIcon, open: folderRustOpenIcon },
  '.github': { closed: folderGithubIcon, open: folderGithubOpenIcon },
  '.git': { closed: folderGitIcon, open: folderGitOpenIcon },
  '.rustup': { closed: folderRustIcon, open: folderRustOpenIcon },
  '.rustup_copy': { closed: folderRustIcon, open: folderRustOpenIcon },
  '.rustup_home': { closed: folderRustIcon, open: folderRustOpenIcon },
  assets: { closed: folderImagesIcon, open: folderImagesOpenIcon },
  components: { closed: folderComponentsIcon, open: folderComponentsOpenIcon },
  crates: { closed: folderRustIcon, open: folderRustOpenIcon },
  css: { closed: folderUiIcon, open: folderUiOpenIcon },
  dist: { closed: folderDistIcon, open: folderDistOpenIcon },
  docs: { closed: folderDocsIcon, open: folderDocsOpenIcon },
  logs: { closed: folderLogIcon, open: folderLogOpenIcon },
  node_modules: { closed: folderNodeIcon, open: folderNodeOpenIcon },
  out: { closed: folderDistIcon, open: folderDistOpenIcon },
  public: { closed: folderPublicIcon, open: folderPublicOpenIcon },
  renderer: { closed: folderUiIcon, open: folderUiOpenIcon },
  scripts: { closed: folderScriptsIcon, open: folderScriptsOpenIcon },
  src: { closed: folderSrcIcon, open: folderSrcOpenIcon },
  'src-tauri': { closed: folderSrcTauriIcon, open: folderSrcTauriOpenIcon },
  tests: { closed: folderTestIcon, open: folderTestOpenIcon },
  utils: { closed: folderUtilsIcon, open: folderUtilsOpenIcon },
}

const FILE_ICONS_BY_EXTENSION: Record<string, string> = {
  css: cssIcon,
  html: htmlIcon,
  jpeg: imageIcon,
  jpg: imageIcon,
  js: javascriptIcon,
  json: jsonIcon,
  lock: lockIcon,
  log: logIcon,
  md: markdownIcon,
  png: imageIcon,
  rs: rustIcon,
  svg: imageIcon,
  toml: tomlIcon,
  ts: typescriptIcon,
  tsx: reactTsIcon,
  yaml: yamlIcon,
  yml: yamlIcon,
}

const FILE_ICONS_BY_NAME: Record<string, string> = {
  'package-lock.json': npmIcon,
  'package.json': npmIcon,
  'tauri.conf.json': tauriIcon,
  'vite.config.ts': viteIcon,
  'vite.config.web.ts': viteIcon,
}

function extension(name: string) {
  const index = name.lastIndexOf('.')
  return index === -1 ? '' : name.slice(index + 1).toLowerCase()
}

function materialFolderIcon(name: string, open: boolean) {
  const icon = FOLDER_ICONS[name] || FOLDER_ICONS[name.toLowerCase()]
  if (!icon) return open ? folderOpenIcon : folderIcon
  return open ? icon.open : icon.closed
}

function materialFileIcon(name: string, fileExtension: string) {
  if (name.endsWith('.config.ts') || name.endsWith('.config.js')) return settingsIcon
  if (name.endsWith('.tsx')) return reactTsIcon
  if (name.endsWith('.jsx')) return reactIcon
  return FILE_ICONS_BY_NAME[name.toLowerCase()] || FILE_ICONS_BY_EXTENSION[fileExtension] || documentIcon
}

function TreeNode({ entry, level }: { entry: WorkspaceTreeEntry; level: number; key?: string }) {
  const [open, setOpen] = useState(level < 1)
  const isDirectory = entry.kind === 'directory'
  const hasChildren = entry.children.length > 0
  const fileExtension = extension(entry.name)
  const fileLabel = FILE_LABEL[fileExtension] || ''
  const materialIcon = isDirectory ? materialFolderIcon(entry.name, open) : materialFileIcon(entry.name, fileExtension)
  const dragText = `@${entry.path}`

  return (
    <div className="workspace-tree-node">
      <button
        type="button"
        className={`workspace-tree-row ${isDirectory ? 'is-directory' : 'is-file'}`}
        style={{ paddingLeft: `${level * 16 + 8}px` }}
        onClick={() => isDirectory && hasChildren && setOpen((current) => !current)}
        title={entry.path}
        draggable
        onDragStart={(event) => {
          event.dataTransfer.setData('application/x-mint-workspace-path', dragText)
          event.dataTransfer.setData('text/plain', dragText)
          event.dataTransfer.effectAllowed = 'copy'
        }}
      >
        <span
          className={`workspace-tree-chevron ${isDirectory && hasChildren ? '' : 'is-spacer'}`}
          data-open={open}
          aria-hidden="true"
        >
          {isDirectory && hasChildren ? '' : null}
        </span>
        <span className={`workspace-tree-icon material-icon ${isDirectory ? 'folder' : fileExtension || 'file'}`} aria-hidden="true">
          {materialIcon ? <img src={materialIcon} alt="" draggable={false} /> : isDirectory ? '' : fileLabel}
        </span>
        <span className="workspace-tree-name">{entry.name}</span>
      </button>
      {isDirectory && open && hasChildren && (
        <div className="workspace-tree-children">
          {entry.children.map((child) => (
            <TreeNode key={child.path} entry={child} level={level + 1} />
          ))}
        </div>
      )}
    </div>
  )
}

export default function WorkspacePanel({ agentMode, sending, workspacePath, onEnableAgentMode, onSetMessage, onWorkspaceReady }: WorkspacePanelProps) {
  const [tree, setTree] = useState<WorkspaceTreeEntry | null>(null)
  const [error, setError] = useState('')

  const refresh = async () => {
    if (!workspacePath.trim()) {
      setError('')
      setTree(null)
      return
    }

    try {
      setError('')
      const nextTree = await getWorkspaceTree(workspacePath)
      setTree(nextTree)
      if (nextTree.path !== workspacePath) onWorkspaceReady(nextTree.path)
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason))
    }
  }

  useEffect(() => {
    refresh()
  }, [workspacePath])

  const startWorkspacePrompt = () => {
    onEnableAgentMode()
    onSetMessage('ดู workspace นี้แล้วช่วยวางแผนงานต่อไปให้หน่อย')
  }

  return (
    <section className="workspace-panel">
      <header className="workspace-panel-header">
        <div>
          <span className="workspace-kicker">Agent Workspace</span>
          <h2>Workspace</h2>
        </div>
        <span className="workspace-agent-pill" data-state={sending ? 'thinking' : agentMode ? 'agent' : 'idle'}>
          {sending ? 'Running' : agentMode ? 'Agent mode' : 'Manual'}
        </span>
      </header>

      <div className="workspace-panel-actions">
        <button type="button" onClick={startWorkspacePrompt}>Use Agent</button>
        <button type="button" onClick={refresh} disabled={!workspacePath.trim()}>Refresh</button>
      </div>

      <div className="workspace-tree-shell">
        {error ? (
          <div className="workspace-tree-empty">{error}</div>
        ) : tree ? (
          <>
            <div className="workspace-root-row">
              <span className="workspace-tree-chevron" data-open="true" aria-hidden="true" />
              <span className="workspace-tree-icon material-icon folder" aria-hidden="true">
                <img src={folderOpenIcon} alt="" draggable={false} />
              </span>
              <span>{tree.name}</span>
            </div>
            <div className="workspace-tree">
              {tree.children.map((entry) => (
                <TreeNode key={entry.path} entry={entry} level={0} />
              ))}
            </div>
          </>
        ) : (
          <div className="workspace-tree-empty">Select a project to show workspace files.</div>
        )}
      </div>
    </section>
  )
}
