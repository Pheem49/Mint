/**
 * shared/utils/syntaxHighlight.tsx
 * Lightweight, zero-dependency syntax highlighter for AI chat code blocks.
 * Provides clean tokenization with a low-saturation aesthetic (Claude / Cursor style).
 */
import React, { ReactNode } from 'react'

interface Token {
  type: string
  text: string
}

// Common bash commands
const BASH_COMMANDS = new Set([
  'ffmpeg', 'ffprobe', 'git', 'npm', 'npx', 'yarn', 'pnpm', 'cargo', 'rustc',
  'docker', 'docker-compose', 'kubectl', 'curl', 'wget', 'cat', 'grep', 'egrep',
  'sed', 'awk', 'find', 'ls', 'cd', 'mkdir', 'rm', 'cp', 'mv', 'touch', 'echo',
  'sudo', 'chmod', 'chown', 'tar', 'unzip', 'zip', 'node', 'python', 'python3',
  'pip', 'pip3', 'tauri', 'vite', 'export', 'source', 'sh', 'bash', 'zsh',
  'systemctl', 'journalctl', 'ps', 'kill', 'killall', 'top', 'htop', 'which',
  'head', 'tail', 'less', 'more', 'nano', 'vim', 'vi', 'ssh', 'scp', 'rsync',
  'brew', 'apt', 'apt-get', 'dnf', 'pacman', 'mint'
])

const JS_KEYWORDS = new Set([
  'const', 'let', 'var', 'function', 'return', 'import', 'export', 'default',
  'from', 'if', 'else', 'for', 'while', 'do', 'switch', 'case', 'break',
  'continue', 'async', 'await', 'class', 'interface', 'type', 'new', 'this',
  'try', 'catch', 'finally', 'throw', 'extends', 'implements', 'typeof',
  'instanceof', 'as', 'in', 'of', 'yield', 'void', 'delete'
])

const PYTHON_KEYWORDS = new Set([
  'def', 'class', 'import', 'from', 'as', 'return', 'if', 'elif', 'else',
  'for', 'while', 'try', 'except', 'finally', 'with', 'yield', 'async',
  'await', 'lambda', 'pass', 'raise', 'in', 'is', 'not', 'and', 'or',
  'global', 'nonlocal', 'assert', 'break', 'continue'
])

const RUST_KEYWORDS = new Set([
  'fn', 'let', 'mut', 'pub', 'struct', 'enum', 'impl', 'trait', 'for',
  'loop', 'while', 'if', 'else', 'match', 'return', 'use', 'mod', 'crate',
  'super', 'self', 'Self', 'where', 'async', 'await', 'move', 'ref', 'type',
  'const', 'static', 'unsafe', 'dyn', 'break', 'continue'
])

const SQL_KEYWORDS = new Set([
  'select', 'from', 'where', 'insert', 'into', 'update', 'delete', 'join',
  'left', 'right', 'inner', 'outer', 'group', 'by', 'order', 'having',
  'limit', 'offset', 'and', 'or', 'not', 'in', 'is', 'null', 'create',
  'table', 'alter', 'drop', 'index', 'primary', 'key', 'foreign', 'references',
  'default', 'values', 'as', 'distinct', 'union', 'all', 'case', 'when',
  'then', 'end', 'like', 'between'
])

function normalizeLang(lang: string): string {
  const l = (lang || '').toLowerCase().trim()
  if (['sh', 'bash', 'zsh', 'shell', 'terminal', 'console', 'cmd'].includes(l)) return 'bash'
  if (['js', 'jsx', 'javascript'].includes(l)) return 'javascript'
  if (['ts', 'tsx', 'typescript'].includes(l)) return 'typescript'
  if (['py', 'python', 'python3'].includes(l)) return 'python'
  if (['rs', 'rust'].includes(l)) return 'rust'
  if (['json'].includes(l)) return 'json'
  if (['sql'].includes(l)) return 'sql'
  if (['html', 'htm', 'xml', 'svg'].includes(l)) return 'html'
  if (['css', 'scss', 'sass', 'less'].includes(l)) return 'css'
  if (['yaml', 'yml'].includes(l)) return 'yaml'
  return 'fallback'
}

/**
 * Tokenizes a single line of Bash / Shell script.
 */
function tokenizeBash(line: string): Token[] {
  const tokens: Token[] = []
  let i = 0
  const len = line.length
  let isFirstWordInStatement = true

  while (i < len) {
    // Comment
    if (line[i] === '#') {
      tokens.push({ type: 'token-comment', text: line.slice(i) })
      break
    }

    // Whitespace
    if (/\s/.test(line[i])) {
      let ws = ''
      while (i < len && /\s/.test(line[i])) {
        ws += line[i]
        i++
      }
      tokens.push({ type: 'token-plain', text: ws })
      continue
    }

    // Single or double quote strings
    if (line[i] === '"' || line[i] === "'") {
      const quote = line[i]
      let str = quote
      i++
      while (i < len && line[i] !== quote) {
        if (line[i] === '\\' && i + 1 < len) {
          str += line[i] + line[i + 1]
          i += 2
        } else {
          str += line[i]
          i++
        }
      }
      if (i < len) {
        str += line[i]
        i++
      }
      tokens.push({ type: 'token-string', text: str })
      isFirstWordInStatement = false
      continue
    }

    // Operators / Pipes / Semicolon
    if (['|', '&', ';', '>', '<'].includes(line[i])) {
      let op = line[i]
      i++
      if (i < len && ['|', '&', '>', '<'].includes(line[i])) {
        op += line[i]
        i++
      }
      tokens.push({ type: 'token-operator', text: op })
      isFirstWordInStatement = true
      continue
    }

    // Variable: $VAR or ${VAR} or $1
    if (line[i] === '$') {
      let v = '$'
      i++
      if (i < len && line[i] === '{') {
        while (i < len && line[i] !== '}') {
          v += line[i]
          i++
        }
        if (i < len) {
          v += line[i]
          i++
        }
      } else {
        while (i < len && /[a-zA-Z0-9_?]/.test(line[i])) {
          v += line[i]
          i++
        }
      }
      tokens.push({ type: 'token-variable', text: v })
      isFirstWordInStatement = false
      continue
    }

    // Flag / option: -i, -vf, --help, -fps=1/10
    if (line[i] === '-' && i + 1 < len && /[a-zA-Z0-9\-]/.test(line[i + 1])) {
      let flag = '-'
      i++
      while (i < len && /[a-zA-Z0-9_\-\.\/]/.test(line[i])) {
        flag += line[i]
        i++
      }
      tokens.push({ type: 'token-flag', text: flag })
      isFirstWordInStatement = false
      continue
    }

    // Word / Identifier / Number
    let word = ''
    while (i < len && !/[\s"'|&;><#]/.test(line[i])) {
      word += line[i]
      i++
    }

    if (isFirstWordInStatement) {
      if (BASH_COMMANDS.has(word) || /^[a-zA-Z0-9_\-\.\/]+$/.test(word)) {
        tokens.push({ type: 'token-command', text: word })
      } else {
        tokens.push({ type: 'token-plain', text: word })
      }
      isFirstWordInStatement = false
    } else {
      if (/^\d+(\.\d+)?$/.test(word)) {
        tokens.push({ type: 'token-number', text: word })
      } else if (word.includes('=')) {
        // e.g. fps=1/10 or KEY=value
        const [key, ...rest] = word.split('=')
        tokens.push({ type: 'token-property', text: key })
        tokens.push({ type: 'token-operator', text: '=' })
        tokens.push({ type: 'token-plain', text: rest.join('=') })
      } else {
        tokens.push({ type: 'token-plain', text: word })
      }
    }
  }

  return tokens
}

/**
 * Tokenizes a single line of JS/TS, Python, Rust, SQL, JSON or general code.
 */
function tokenizeGeneral(line: string, lang: string): Token[] {
  const tokens: Token[] = []
  let i = 0
  const len = line.length

  while (i < len) {
    // Single-line comments
    if (
      (line[i] === '/' && line[i + 1] === '/') ||
      ((lang === 'python' || lang === 'yaml') && line[i] === '#') ||
      (lang === 'sql' && line[i] === '-' && line[i + 1] === '-')
    ) {
      tokens.push({ type: 'token-comment', text: line.slice(i) })
      break
    }

    // Whitespace
    if (/\s/.test(line[i])) {
      let ws = ''
      while (i < len && /\s/.test(line[i])) {
        ws += line[i]
        i++
      }
      tokens.push({ type: 'token-plain', text: ws })
      continue
    }

    // Strings
    if (line[i] === '"' || line[i] === "'" || line[i] === '`') {
      const quote = line[i]
      let str = quote
      i++
      while (i < len && line[i] !== quote) {
        if (line[i] === '\\' && i + 1 < len) {
          str += line[i] + line[i + 1]
          i += 2
        } else {
          str += line[i]
          i++
        }
      }
      if (i < len) {
        str += line[i]
        i++
      }
      // In JSON, check if it's followed by a colon (object key)
      let isKey = false
      if (lang === 'json') {
        let peek = i
        while (peek < len && /\s/.test(line[peek])) peek++
        if (peek < len && line[peek] === ':') isKey = true
      }
      tokens.push({ type: isKey ? 'token-property' : 'token-string', text: str })
      continue
    }

    // Numbers
    if (/\d/.test(line[i])) {
      let num = ''
      while (i < len && /[0-9a-fA-FxX\._]/.test(line[i])) {
        num += line[i]
        i++
      }
      tokens.push({ type: 'token-number', text: num })
      continue
    }

    // Words / Identifiers / Keywords
    if (/[a-zA-Z_$]/.test(line[i])) {
      let word = ''
      while (i < len && /[a-zA-Z0-9_$]/.test(line[i])) {
        word += line[i]
        i++
      }

      // Check keyword sets
      const lower = word.toLowerCase()
      if (
        (lang === 'javascript' || lang === 'typescript') &&
        JS_KEYWORDS.has(word)
      ) {
        tokens.push({ type: 'token-keyword', text: word })
      } else if (lang === 'python' && PYTHON_KEYWORDS.has(word)) {
        tokens.push({ type: 'token-keyword', text: word })
      } else if (lang === 'rust' && RUST_KEYWORDS.has(word)) {
        tokens.push({ type: 'token-keyword', text: word })
      } else if (lang === 'sql' && SQL_KEYWORDS.has(lower)) {
        tokens.push({ type: 'token-keyword', text: word })
      } else if (['true', 'false', 'null', 'undefined', 'None', 'True', 'False'].includes(word)) {
        tokens.push({ type: 'token-boolean', text: word })
      } else {
        // Function call check
        let peek = i
        while (peek < len && /\s/.test(line[peek])) peek++
        if (peek < len && line[peek] === '(') {
          tokens.push({ type: 'token-function', text: word })
        } else if (/^[A-Z][a-zA-Z0-9_]*$/.test(word)) {
          tokens.push({ type: 'token-type', text: word })
        } else {
          tokens.push({ type: 'token-plain', text: word })
        }
      }
      continue
    }

    // Punctuation & Operators
    tokens.push({ type: 'token-punctuation', text: line[i] })
    i++
  }

  return tokens
}

/**
 * Tokenize a line of code based on target language.
 */
export function tokenizeLine(line: string, language: string): Token[] {
  const norm = normalizeLang(language)
  if (norm === 'bash') {
    return tokenizeBash(line)
  }
  return tokenizeGeneral(line, norm)
}

/**
 * Highlight and format code into an array of React lines with line numbers & tokens.
 */
export function renderHighlightedCode(
  code: string,
  language: string,
  showLineNumbers = true
): ReactNode {
  const lines = code.split('\n')

  return (
    <div className="chat-code-lines">
      {lines.map((line, idx) => {
        const tokens = tokenizeLine(line, language)
        return (
          <div key={idx} className="chat-code-line">
            {showLineNumbers && (
              <span className="chat-code-line-num" aria-hidden="true">
                {idx + 1}
              </span>
            )}
            <span className="chat-code-line-content">
              {tokens.length === 0 ? (
                '\n'
              ) : (
                tokens.map((tok, tIdx) => (
                  <span key={tIdx} className={tok.type}>
                    {tok.text}
                  </span>
                ))
              )}
            </span>
          </div>
        )
      })}
    </div>
  )
}
