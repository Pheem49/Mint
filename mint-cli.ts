#!/usr/bin/env node
import dotenv from 'dotenv'
import pkg from './package.json'
dotenv.config({ quiet: true })

// Suppress experimental SQLite warning
const originalEmit = process.emit
process.emit = function (name: any, data: any, ...args: any[]) {
    if (name === 'warning' && typeof data === 'object' &&
        data.name === 'ExperimentalWarning' && data.message.includes('SQLite')) {
        return false
    }
    return originalEmit.apply(process, [name, data, ...args] as any)
} as any

import { Command } from 'commander'

// ── CLI modules ──────────────────────────────────────────────────────────────
import { colors, exitWithGoodbye } from './src/CLI/cli_colors'
import { formatProgress } from './src/CLI/cli_formatters'
import { startInteractiveChat } from './src/CLI/interactive_chat'
import { requestCodeApproval } from './src/CLI/approval_handler'
import { learnSkillFile } from './src/CLI/skill_manager'

// ── Feature / system modules ────────────────────────────────────────────────
import { runOnboarding } from './src/CLI/onboarding'
import { startAgent } from './src/AI_Brain/headless_agent'
import { displayFeatures } from './src/CLI/list_features'
import { readConfig, writeConfig } from './src/System/config_manager'
import { executeCodeTask } from './src/CLI/code_agent'
import { runUpdate, runStartupAutoUpdate, shouldRunAutoUpdate } from './src/CLI/updater'
import { runGmailAuth } from './src/CLI/gmail_auth'
import { loadImageAsDataUri } from './src/CLI/image_input'
import { summarizeRepository, formatRepoSummary } from './src/CLI/repo_summarizer'
import { buildSymbolIndex, formatSymbolIndex } from './src/CLI/symbol_indexer'
import {
    indexSemanticCode,
    searchSemanticCode,
    formatSemanticCodeIndex,
    formatSemanticCodeSearch
} from './src/CLI/semantic_code_search'

const memoryStore = require('./src/AI_Brain/memory_store')

// ── Startup banner ───────────────────────────────────────────────────────────
const startupConfig   = readConfig()
const startupProvider = startupConfig.aiProvider || 'gemini'
const startupModel    = startupProvider === 'openai'
    ? (startupConfig.openaiModel || 'gpt-4o')
    : startupProvider === 'anthropic'
        ? (startupConfig.anthropicModel || 'claude-3-5-sonnet-latest')
        : startupProvider === 'local_openai'
            ? (startupConfig.localModelName || 'local-model')
            : startupProvider === 'ollama'
                ? (startupConfig.ollamaModel || 'llama3:latest')
                : (startupConfig.geminiModel || 'gemini-2.5-flash')

const startupNow  = new Date()
const startupTime = startupNow.toLocaleString('th-TH', {
    day: '2-digit', month: '2-digit', year: 'numeric',
    hour: '2-digit', minute: '2-digit', hour12: false
}).replace(',', '')
console.log(`${colors.mint}[Mint] v${pkg.version} | ${startupTime} | Active AI: ${startupProvider} • ${startupModel}${colors.reset}`)

process.once('SIGINT', () => exitWithGoodbye(0))

// ── Commander program ────────────────────────────────────────────────────────
const program = new Command()

program
    .name('mint')
    .description('Mint - Your Personal AI Assistant CLI')
    .version(pkg.version)

// Auto-update hook
program.hook('preAction', async (thisCommand, actionCommand) => {
    if (actionCommand.name() === 'update' || process.env.MINT_SKIP_AUTO_UPDATE === '1') return
    const config = readConfig()
    if (config.enableAutoUpdate === false || !shouldRunAutoUpdate(config)) return
    console.log(`${colors.gray}[Mint Update] Checking for updates...${colors.reset}`)
    const result = await runStartupAutoUpdate(config, writeConfig)
    if (result.status === 'updated') {
        console.log(`${colors.mint}[Mint Update] ${result.message}${colors.reset}`)
    } else if (result.status === 'error') {
        console.log(`${colors.gray}[Mint Update] ${result.message}${colors.reset}`)
    }
})

// ── Commands ─────────────────────────────────────────────────────────────────

program
    .command('chat', { isDefault: true })
    .description('Start interactive chat session with Mint')
    .argument('[message]', 'Initial message to send to Mint')
    .option('-i, --image <path>', 'Attach an image file to the initial message')
    .action(async (message, options) => {
        await startInteractiveChat(message, { imagePath: options.image })
    })

program
    .command('onboard')
    .description('Setup Mint for the first time')
    .option('--install-daemon', 'Automatically install systemd background agent')
    .action(async (options) => {
        await runOnboarding(options)
    })

program
    .command('agent')
    .description('Run Mint as a background agent (headless)')
    .argument('[initialTask]', 'Optional first task to perform immediately on startup')
    .action(async (initialTask) => {
        if (initialTask) {
            const taskManager = require('./src/System/task_manager')
            taskManager.addTask(initialTask)
            console.log(`\n${colors.mint}${colors.bright}[Mint-Agent] Starting with initial task:${colors.reset} "${initialTask}"`)
        }
        await startAgent()
    })

program
    .command('list')
    .description('Show list of Mint features and commands')
    .action(() => displayFeatures())

program
    .command('summarize')
    .alias('summary')
    .description('Summarize a repository structure, tooling, git state, and key files')
    .argument('[path]', 'Repository path to summarize', process.cwd())
    .option('--json', 'Print raw JSON summary')
    .action((targetPath, options) => {
        try {
            const summary = summarizeRepository(targetPath)
            if (options.json) { console.log(JSON.stringify(summary, null, 2)); return; }
            console.log(`\n${formatRepoSummary(summary)}\n`)
        } catch (error: any) {
            console.error(`\n${colors.pink}Summarize failed:${colors.reset} ${error.message}\n`)
            process.exitCode = 1
        }
    })

program
    .command('symbols')
    .alias('symbol-index')
    .description('Build a source symbol index for the current repository')
    .argument('[path]', 'Repository path to index', process.cwd())
    .option('--json', 'Print raw JSON symbol index')
    .option('--limit <count>', 'Limit formatted symbols shown', value => Number(value), 80)
    .action((targetPath, options) => {
        try {
            const index = buildSymbolIndex(targetPath)
            if (options.json) { console.log(JSON.stringify(index, null, 2)); return; }
            console.log(`\n${formatSymbolIndex(index, { limit: options.limit })}\n`)
        } catch (error: any) {
            console.error(`\n${colors.pink}Symbol index failed:${colors.reset} ${error.message}\n`)
            process.exitCode = 1
        }
    })

const semanticCodeCommand = program
    .command('semantic-code')
    .alias('semantic')
    .description('Index and search source code semantically with embeddings')

semanticCodeCommand
    .command('index')
    .description('Create embeddings for source code chunks in a repository')
    .argument('[path]', 'Repository path to index', process.cwd())
    .option('--json', 'Print raw JSON index metadata')
    .action(async (targetPath, options) => {
        try {
            const index = await indexSemanticCode(targetPath, {
                onProgress: (info: any) => {
                    if (info.current === 1 || info.current === info.total || info.current % 25 === 0) {
                        console.log(`${colors.gray}[Semantic Code] Embedded ${info.current}/${info.total}: ${info.file}${colors.reset}`)
                    }
                }
            })
            if (options.json) { console.log(JSON.stringify(index, null, 2)); return; }
            console.log(`\n${formatSemanticCodeIndex(index)}\n`)
        } catch (error: any) {
            console.error(`\n${colors.pink}Semantic code index failed:${colors.reset} ${error.message}\n`)
            process.exitCode = 1
        }
    })

semanticCodeCommand
    .command('search')
    .description('Search an existing semantic code index')
    .argument('<query...>', 'Natural language code search query')
    .option('--path <path>', 'Repository path to search', process.cwd())
    .option('--json', 'Print raw JSON search results')
    .option('--top-k <count>', 'Number of results to return', value => Number(value), 5)
    .action(async (query, options) => {
        try {
            const results = await searchSemanticCode(query.join(' '), options.path, { topK: options.topK })
            if (options.json) { console.log(JSON.stringify(results, null, 2)); return; }
            console.log(`\n${formatSemanticCodeSearch(results)}\n`)
        } catch (error: any) {
            console.error(`\n${colors.pink}Semantic code search failed:${colors.reset} ${error.message}\n`)
            process.exitCode = 1
        }
    })

program
    .command('learn')
    .description('Read a local markdown/text file and remember it as a Mint skill')
    .argument('[filePath]', 'Path to a .md or .txt skill/instruction file')
    .option('--delete <idOrPathOrName>', 'Delete a learned skill by id, path, or name')
    .option('--list', 'List learned skills')
    .action((filePath, options) => {
        try {
            if (options.list) {
                const skills = memoryStore.getLearnedSkills(50)
                if (skills.length === 0) { console.log(`\n${colors.gray}No learned skills stored.${colors.reset}\n`); return; }
                console.log(`\n${colors.bright}Learned Skills:${colors.reset}`)
                skills.forEach((skill: any) => {
                    console.log(`${colors.mint}#${skill.id}${colors.reset} ${skill.name}`)
                    console.log(`  ${colors.gray}${skill.source_path}${colors.reset}`)
                })
                console.log('')
                return
            }
            if (options.delete) {
                const deleted = memoryStore.deleteLearnedSkill(options.delete)
                if (deleted > 0) {
                    console.log(`\n${colors.mint}✓${colors.reset} Deleted learned skill: ${options.delete}\n`)
                } else {
                    console.log(`\n${colors.pink}✗${colors.reset} Learned skill not found: ${options.delete}\n`)
                    process.exitCode = 1
                }
                return
            }
            if (!filePath) throw new Error('Usage: mint learn <path-to-skill.md>')

            const learned = learnSkillFile(filePath)
            console.log(`\n${colors.mint}✓${colors.reset} Learned skill: ${learned.name}`)
            console.log(`${colors.gray}Path: ${learned.source_path}${colors.reset}`)
            if (learned.stored_length < learned.content_length) {
                console.log(`${colors.gray}Stored first ${learned.stored_length} of ${learned.content_length} characters.${colors.reset}`)
            }
            console.log('')
        } catch (error: any) {
            console.error(`\n${colors.pink}Learn failed:${colors.reset} ${error.message}\n`)
            process.exitCode = 1
        }
    })

program
    .command('task')
    .description('Delegate a complex task to the background agent')
    .argument('<description>', 'Description of the task for Mint to perform autonomously')
    .action(async (description) => {
        const taskManager = require('./src/System/task_manager')
        const task = taskManager.addTask(description)
        console.log(`\n${colors.mint}${colors.bright}Task Received!${colors.reset}`)
        console.log(`${colors.gray}Task ID: ${task.id}${colors.reset}`)
        console.log(`"${description}"`)
        console.log(`\n${colors.cyan}Mint Agent is starting to work on this in the background.${colors.reset}`)
        console.log(`${colors.gray}You will receive a notification when it's done.${colors.reset}\n`)
    })

program
    .command('update')
    .description('Check for and install the latest Mint CLI version from npm')
    .option('--check',   'Only check whether an update is available')
    .option('--dry-run', 'Show the npm update operation without installing')
    .action(async (options) => {
        console.log(`\n${colors.mint}${colors.bright}[Mint Update]${colors.reset} Checking npm for updates...`)
        try {
            const result = await runUpdate({
                checkOnly: options.check   === true,
                dryRun:    options.dryRun  === true
            })
            const color = result.status === 'error' ? colors.pink : colors.mint
            console.log(`${color}${result.message}${colors.reset}\n`)
            if (result.status === 'error') process.exitCode = 1
        } catch (error: any) {
            console.error(`${colors.pink}Update failed: ${error.message}${colors.reset}\n`)
            process.exitCode = 1
        }
    })

program
    .command('mcp')
    .description('Manage MCP (Model Context Protocol) servers')
    .addCommand(new Command('add')
        .description('Add a new MCP server')
        .argument('<name>',    'Server name')
        .argument('<command>', 'Command to run (e.g. npx)')
        .option('-a, --args <args...>', 'Command arguments')
        .option('-e, --env <env...>',   'Environment variables (KEY=VALUE)')
        .action((name, command, options) => {
            const config     = readConfig()
            const mcpServers = config.mcpServers || {}
            const env: Record<string, string> = {}
            if (options.env) {
                options.env.forEach((kv: string) => {
                    const [k, v] = kv.split('=')
                    if (k && v) env[k] = v
                })
            }
            mcpServers[name] = { command, args: options.args || [], env }
            config.mcpServers = mcpServers
            writeConfig(config)
            console.log(`\n${colors.mint}✓${colors.reset} MCP server "${name}" added successfully.`)
        })
    )
    .addCommand(new Command('remove')
        .description('Remove an MCP server')
        .argument('<name>', 'Server name')
        .action((name) => {
            const config = readConfig()
            if (config.mcpServers && config.mcpServers[name]) {
                delete config.mcpServers[name]
                writeConfig(config)
                console.log(`\n${colors.mint}✓${colors.reset} MCP server "${name}" removed.`)
            } else {
                console.log(`\n${colors.pink}✗${colors.reset} MCP server "${name}" not found.`)
            }
        })
    )
    .addCommand(new Command('list')
        .description('List configured MCP servers')
        .action(() => {
            const config  = readConfig()
            const servers = Object.keys(config.mcpServers || {})
            if (servers.length === 0) {
                console.log(`\n${colors.gray}No MCP servers configured.${colors.reset}`)
            } else {
                console.log(`\n${colors.bright}Configured MCP Servers:${colors.reset}`)
                servers.forEach(name => {
                    const s = config.mcpServers[name]
                    console.log(`${colors.mint}• ${colors.bright}${name}${colors.reset}`)
                    console.log(`  ${colors.gray}Command:${colors.reset} ${s.command} ${(s.args || []).join(' ')}`)
                })
            }
        })
    )
    .addCommand(new Command('clear')
        .description('Remove all MCP servers')
        .action(() => {
            const config = readConfig()
            config.mcpServers = {}
            writeConfig(config)
            console.log(`\n${colors.mint}✓${colors.reset} All MCP servers cleared.`)
        })
    )

program
    .command('gmail')
    .description('Manage Gmail integration')
    .addCommand(new Command('auth')
        .description('Open Google OAuth login and save a Gmail refresh token')
        .option('--port <port>', 'Local callback port, defaults to a random available port')
        .option('--no-open',     'Print the auth link without opening a browser')
        .action(async (options) => {
            try {
                const result = await runGmailAuth({
                    port:        options.port ? Number(options.port) : 0,
                    openBrowser: options.open,
                    logger:      console
                })
                console.log(`\n${colors.mint}✓${colors.reset} Gmail connected for ${result.userId}. Refresh token saved.`)
                console.log(`${colors.gray}Scopes: ${result.scopes.join(', ')}${colors.reset}\n`)
            } catch (error: any) {
                console.error(`\n${colors.pink}Gmail auth failed:${colors.reset} ${error.message}\n`)
                process.exitCode = 1
            }
        })
    )

program
    .command('code')
    .description('Run Mint in workspace-aware coding mode for the current project')
    .argument('<task>', 'Coding task to execute in the current working directory')
    .option('-i, --image <path>', 'Attach an image file as context for the coding task')
    .action(async (task, options) => {
        console.log(`\n${colors.mint}${colors.bright}[Mint Code]${colors.reset} Workspace: ${process.cwd()}`)
        try {
            let image = null
            if (options.image) {
                image = loadImageAsDataUri(options.image)
                console.log(`${colors.gray}[Mint Code] Image: ${image.path}${colors.reset}`)
            }
            console.log(`${colors.gray}[Mint Code] Task: ${task}${colors.reset}\n`)

            const result: any = await executeCodeTask(task, {
                cwd:           process.cwd(),
                imageDataUri:  image ? image.dataUri : null,
                imagePath:     image ? image.path    : null,
                onProgress:    (info: any) => console.log(formatProgress(info)),
                requestApproval: requestCodeApproval
            })

            console.log(`\n${colors.mint}${colors.bright}Summary${colors.reset}`)
            console.log(result.summary)
            console.log(`\n${colors.cyan}Verification:${colors.reset} ${result.verification}`)
            console.log(`${colors.gray}Completed in ${result.steps} step(s).${colors.reset}\n`)
        } catch (error: any) {
            console.error(`\n${colors.pink}[Mint Code Error]${colors.reset} ${error.message}\n`)
            process.exitCode = 1
        }
    })

// ── Parse ────────────────────────────────────────────────────────────────────
program.parseAsync(process.argv).catch((error) => {
    console.error(`${colors.pink}${error.message}${colors.reset}`)
    process.exitCode = 1
})
