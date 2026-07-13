/**
 * shared/utils/fileIcons.ts
 * Utility mapping and resolution functions for Material Icon Theme icons.
 * Extracted from WorkspacePanel to be shared across multiple components (e.g. AgentActivityTable).
 */

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

export const FOLDER_ICONS: Record<string, { closed: string; open: string }> = {
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

export const FILE_ICONS_BY_EXTENSION: Record<string, string> = {
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

export const FILE_ICONS_BY_NAME: Record<string, string> = {
  'package-lock.json': npmIcon,
  'package.json': npmIcon,
  'tauri.conf.json': tauriIcon,
  'vite.config.ts': viteIcon,
  'vite.config.web.ts': viteIcon,
}

export function getExtension(name: string): string {
  const index = name.lastIndexOf('.')
  return index === -1 ? '' : name.slice(index + 1).toLowerCase()
}

export function materialFolderIcon(name: string, open: boolean): string {
  const icon = FOLDER_ICONS[name] || FOLDER_ICONS[name.toLowerCase()]
  if (!icon) return open ? folderOpenIcon : folderIcon
  return open ? icon.open : icon.closed
}

export function materialFileIcon(name: string, fileExtension: string): string {
  if (name.endsWith('.config.ts') || name.endsWith('.config.js')) return settingsIcon
  if (name.endsWith('.tsx')) return reactTsIcon
  if (name.endsWith('.jsx')) return reactIcon
  return FILE_ICONS_BY_NAME[name.toLowerCase()] || FILE_ICONS_BY_EXTENSION[fileExtension] || documentIcon
}

export { folderIcon, folderOpenIcon }

