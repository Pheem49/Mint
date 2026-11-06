/**
 * shared/utils/fileIcons.ts
 * Utility mapping and resolution functions for Material Icon Theme icons.
 * Extracted from WorkspacePanel to be shared across multiple components (e.g. AgentActivityTable, TurnFileChanges).
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

// Language & file icons
import audioIcon from 'material-icon-theme/icons/audio.svg?url'
import cIcon from 'material-icon-theme/icons/c.svg?url'
import consoleIcon from 'material-icon-theme/icons/console.svg?url'
import cppIcon from 'material-icon-theme/icons/cpp.svg?url'
import csharpIcon from 'material-icon-theme/icons/csharp.svg?url'
import cssIcon from 'material-icon-theme/icons/css.svg?url'
import dartIcon from 'material-icon-theme/icons/dart.svg?url'
import databaseIcon from 'material-icon-theme/icons/database.svg?url'
import dockerIcon from 'material-icon-theme/icons/docker.svg?url'
import documentIcon from 'material-icon-theme/icons/document.svg?url'
import fontIcon from 'material-icon-theme/icons/font.svg?url'
import gitIcon from 'material-icon-theme/icons/git.svg?url'
import goIcon from 'material-icon-theme/icons/go.svg?url'
import graphqlIcon from 'material-icon-theme/icons/graphql.svg?url'
import htmlIcon from 'material-icon-theme/icons/html.svg?url'
import imageIcon from 'material-icon-theme/icons/image.svg?url'
import javaIcon from 'material-icon-theme/icons/java.svg?url'
import javascriptIcon from 'material-icon-theme/icons/javascript.svg?url'
import jsonIcon from 'material-icon-theme/icons/json.svg?url'
import jupyterIcon from 'material-icon-theme/icons/jupyter.svg?url'
import kotlinIcon from 'material-icon-theme/icons/kotlin.svg?url'
import lockIcon from 'material-icon-theme/icons/lock.svg?url'
import logIcon from 'material-icon-theme/icons/log.svg?url'
import luaIcon from 'material-icon-theme/icons/lua.svg?url'
import markdownIcon from 'material-icon-theme/icons/markdown.svg?url'
import npmIcon from 'material-icon-theme/icons/npm.svg?url'
import pdfIcon from 'material-icon-theme/icons/pdf.svg?url'
import phpIcon from 'material-icon-theme/icons/php.svg?url'
import powershellIcon from 'material-icon-theme/icons/powershell.svg?url'
import pythonIcon from 'material-icon-theme/icons/python.svg?url'
import rIcon from 'material-icon-theme/icons/r.svg?url'
import reactIcon from 'material-icon-theme/icons/react.svg?url'
import reactTsIcon from 'material-icon-theme/icons/react_ts.svg?url'
import rubyIcon from 'material-icon-theme/icons/ruby.svg?url'
import rustIcon from 'material-icon-theme/icons/rust.svg?url'
import sassIcon from 'material-icon-theme/icons/sass.svg?url'
import scalaIcon from 'material-icon-theme/icons/scala.svg?url'
import settingsIcon from 'material-icon-theme/icons/settings.svg?url'
import svelteIcon from 'material-icon-theme/icons/svelte.svg?url'
import swiftIcon from 'material-icon-theme/icons/swift.svg?url'
import tauriIcon from 'material-icon-theme/icons/tauri.svg?url'
import tomlIcon from 'material-icon-theme/icons/toml.svg?url'
import typescriptIcon from 'material-icon-theme/icons/typescript.svg?url'
import videoIcon from 'material-icon-theme/icons/video.svg?url'
import viteIcon from 'material-icon-theme/icons/vite.svg?url'
import vueIcon from 'material-icon-theme/icons/vue.svg?url'
import xmlIcon from 'material-icon-theme/icons/xml.svg?url'
import yamlIcon from 'material-icon-theme/icons/yaml.svg?url'
import zipIcon from 'material-icon-theme/icons/zip.svg?url'

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
  // Web & Styles
  css: cssIcon,
  scss: sassIcon,
  sass: sassIcon,
  less: cssIcon,
  html: htmlIcon,
  htm: htmlIcon,
  graphql: graphqlIcon,
  gql: graphqlIcon,
  vue: vueIcon,
  svelte: svelteIcon,

  // JavaScript & TypeScript
  js: javascriptIcon,
  mjs: javascriptIcon,
  cjs: javascriptIcon,
  jsx: reactIcon,
  ts: typescriptIcon,
  mts: typescriptIcon,
  cts: typescriptIcon,
  tsx: reactTsIcon,

  // Python & Data Science
  py: pythonIcon,
  pyc: pythonIcon,
  pyd: pythonIcon,
  pyw: pythonIcon,
  ipynb: jupyterIcon,

  // Systems & Compiled Languages
  rs: rustIcon,
  c: cIcon,
  h: cIcon,
  cpp: cppIcon,
  cc: cppIcon,
  cxx: cppIcon,
  hpp: cppIcon,
  hh: cppIcon,
  hxx: cppIcon,
  cs: csharpIcon,
  go: goIcon,
  java: javaIcon,
  jar: javaIcon,
  kt: kotlinIcon,
  kts: kotlinIcon,
  php: phpIcon,
  rb: rubyIcon,
  swift: swiftIcon,
  scala: scalaIcon,
  dart: dartIcon,
  lua: luaIcon,
  r: rIcon,

  // Shell & Scripts
  sh: consoleIcon,
  bash: consoleIcon,
  zsh: consoleIcon,
  fish: consoleIcon,
  ps1: powershellIcon,
  psm1: powershellIcon,
  bat: consoleIcon,
  cmd: consoleIcon,

  // Data, Config & Docs
  json: jsonIcon,
  json5: jsonIcon,
  jsonc: jsonIcon,
  yaml: yamlIcon,
  yml: yamlIcon,
  toml: tomlIcon,
  xml: xmlIcon,
  sql: databaseIcon,
  db: databaseIcon,
  sqlite: databaseIcon,
  sqlite3: databaseIcon,
  md: markdownIcon,
  mdx: markdownIcon,
  pdf: pdfIcon,
  log: logIcon,
  lock: lockIcon,

  // Media, Fonts & Archives
  png: imageIcon,
  jpg: imageIcon,
  jpeg: imageIcon,
  gif: imageIcon,
  webp: imageIcon,
  svg: imageIcon,
  ico: imageIcon,
  mp3: audioIcon,
  wav: audioIcon,
  ogg: audioIcon,
  mp4: videoIcon,
  webm: videoIcon,
  mov: videoIcon,
  zip: zipIcon,
  tar: zipIcon,
  gz: zipIcon,
  tgz: zipIcon,
  '7z': zipIcon,
  rar: zipIcon,
  woff: fontIcon,
  woff2: fontIcon,
  ttf: fontIcon,
  otf: fontIcon,
}

export const FILE_ICONS_BY_NAME: Record<string, string> = {
  'package-lock.json': npmIcon,
  'package.json': npmIcon,
  'tauri.conf.json': tauriIcon,
  'vite.config.ts': viteIcon,
  'vite.config.web.ts': viteIcon,
  'vite.config.js': viteIcon,
  'dockerfile': dockerIcon,
  'containerfile': dockerIcon,
  'docker-compose.yml': dockerIcon,
  'docker-compose.yaml': dockerIcon,
  '.gitignore': gitIcon,
  '.gitattributes': gitIcon,
  '.gitmodules': gitIcon,
  '.env': settingsIcon,
  '.env.local': settingsIcon,
  '.env.development': settingsIcon,
  '.env.production': settingsIcon,
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
  const lowerName = name.toLowerCase()
  if (FILE_ICONS_BY_NAME[lowerName]) return FILE_ICONS_BY_NAME[lowerName]
  if (lowerName.endsWith('.config.ts') || lowerName.endsWith('.config.js') || lowerName.endsWith('.config.mjs')) return settingsIcon
  if (lowerName.endsWith('.tsx')) return reactTsIcon
  if (lowerName.endsWith('.jsx')) return reactIcon
  return FILE_ICONS_BY_EXTENSION[fileExtension.toLowerCase()] || documentIcon
}

export { folderIcon, folderOpenIcon, documentIcon }
