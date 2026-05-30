import * as fs from 'fs'
import * as path from 'path'

const IGNORED_DIRS = new Set([
    '.git',
    '.cache',
    '.next',
    '.nuxt',
    'coverage',
    'dist',
    'build',
    'out',
    'node_modules'
]);

const LANGUAGE_BY_EXT = {
    '.cjs': 'JavaScript',
    '.js': 'JavaScript',
    '.jsx': 'JavaScript',
    '.mjs': 'JavaScript',
    '.py': 'Python',
    '.rs': 'Rust',
    '.ts': 'TypeScript',
    '.tsx': 'TypeScript'
};

const SOURCE_EXTENSIONS = new Set(Object.keys(LANGUAGE_BY_EXT));

function walkSourceFiles(root: string, options: any = {}) {
    const maxFiles = options.maxFiles || 2500;
    const files = [];

    function visit(dir) {
        let entries = [];
        try {
            entries = fs.readdirSync(dir, { withFileTypes: true });
        } catch (_) {
            return;
        }

        entries.sort((a, b) => a.name.localeCompare(b.name));
        for (const entry of entries) {
            if (files.length >= maxFiles) return;
            const fullPath = path.join(dir, entry.name);
            const relativePath = path.relative(root, fullPath);

            if (entry.isDirectory()) {
                if (IGNORED_DIRS.has(entry.name)) continue;
                visit(fullPath);
                continue;
            }

            if (entry.isFile() && SOURCE_EXTENSIONS.has(path.extname(entry.name).toLowerCase())) {
                files.push(relativePath);
            }
        }
    }

    visit(root);
    return files;
}

function makeSymbol({ name, kind, file, line, column, language, signature }) {
    return {
        name,
        kind,
        file,
        line,
        column,
        language,
        signature: signature.trim()
    };
}

function scanPattern(lines, file, language, pattern, kind, symbols) {
    lines.forEach((lineText, index) => {
        const match = lineText.match(pattern);
        if (!match) return;

        const name = match.groups?.name || match[1];
        if (!name) return;

        symbols.push(makeSymbol({
            name,
            kind,
            file,
            line: index + 1,
            column: lineText.indexOf(name) + 1,
            language,
            signature: lineText.trim()
        }));
    });
}

function indexJavaScriptLike(content, file, language) {
    const lines = content.split('\n');
    const symbols = [];

    scanPattern(lines, file, language, /^\s*(?:export\s+)?(?:async\s+)?function\*?\s+(?<name>[A-Za-z_$][\w$]*)\s*\(/, 'function', symbols);
    scanPattern(lines, file, language, /^\s*(?:export\s+)?class\s+(?<name>[A-Za-z_$][\w$]*)\b/, 'class', symbols);
    scanPattern(lines, file, language, /^\s*(?:export\s+)?(?:const|let|var)\s+(?<name>[A-Za-z_$][\w$]*)\s*=\s*(?:async\s*)?(?:\([^)]*\)|[A-Za-z_$][\w$]*)\s*=>/, 'function', symbols);
    scanPattern(lines, file, language, /^\s*(?:export\s+)?(?:const|let|var)\s+(?<name>[A-Za-z_$][\w$]*)\s*=\s*(?:async\s+)?function\b/, 'function', symbols);
    scanPattern(lines, file, language, /^\s*(?:export\s+)?interface\s+(?<name>[A-Za-z_$][\w$]*)\b/, 'interface', symbols);
    scanPattern(lines, file, language, /^\s*(?:export\s+)?type\s+(?<name>[A-Za-z_$][\w$]*)\b/, 'type', symbols);
    scanPattern(lines, file, language, /^\s*(?:export\s+)?enum\s+(?<name>[A-Za-z_$][\w$]*)\b/, 'enum', symbols);
    scanPattern(lines, file, language, /^\s*(?:module\.)?exports\.(?<name>[A-Za-z_$][\w$]*)\s*=/, 'export', symbols);

    return symbols;
}

function indexPython(content, file) {
    const lines = content.split('\n');
    const symbols = [];

    scanPattern(lines, file, 'Python', /^\s*def\s+(?<name>[A-Za-z_]\w*)\s*\(/, 'function', symbols);
    scanPattern(lines, file, 'Python', /^\s*async\s+def\s+(?<name>[A-Za-z_]\w*)\s*\(/, 'function', symbols);
    scanPattern(lines, file, 'Python', /^\s*class\s+(?<name>[A-Za-z_]\w*)\b/, 'class', symbols);

    return symbols;
}

function indexRust(content, file) {
    const lines = content.split('\n');
    const symbols = [];

    scanPattern(lines, file, 'Rust', /^\s*(?:pub\s+)?(?:async\s+)?fn\s+(?<name>[A-Za-z_]\w*)\s*\(/, 'function', symbols);
    scanPattern(lines, file, 'Rust', /^\s*(?:pub\s+)?struct\s+(?<name>[A-Za-z_]\w*)\b/, 'struct', symbols);
    scanPattern(lines, file, 'Rust', /^\s*(?:pub\s+)?enum\s+(?<name>[A-Za-z_]\w*)\b/, 'enum', symbols);
    scanPattern(lines, file, 'Rust', /^\s*(?:pub\s+)?trait\s+(?<name>[A-Za-z_]\w*)\b/, 'trait', symbols);
    scanPattern(lines, file, 'Rust', /^\s*impl(?:\s+\w+)?\s+for\s+(?<name>[A-Za-z_]\w*)\b/, 'impl', symbols);

    return symbols;
}

function indexFileSymbols(root, relativePath) {
    const fullPath = path.join(root, relativePath);
    const ext = path.extname(relativePath).toLowerCase();
    const language = LANGUAGE_BY_EXT[ext] || 'Other';
    let content = '';

    try {
        content = fs.readFileSync(fullPath, 'utf8');
    } catch (_) {
        return [];
    }

    if (language === 'Python') return indexPython(content, relativePath);
    if (language === 'Rust') return indexRust(content, relativePath);
    return indexJavaScriptLike(content, relativePath, language);
}

function countBy(items, key) {
    const counts = new Map();
    for (const item of items) {
        const value = item[key] || 'unknown';
        counts.set(value, (counts.get(value) || 0) + 1);
    }
    return Array.from(counts.entries())
        .map(([name, count]) => ({ name, count }))
        .sort((a, b) => b.count - a.count || a.name.localeCompare(b.name));
}

function buildSymbolIndex(targetPath = process.cwd(), options: any = {}) {
    const root = path.resolve(targetPath);
    const stat = fs.statSync(root);
    if (!stat.isDirectory()) {
        throw new Error(`Symbol index path is not a directory: ${root}`);
    }

    const files = walkSourceFiles(root, options);
    const symbols = files.flatMap(file => indexFileSymbols(root, file));

    return {
        root,
        fileCount: files.length,
        indexedFiles: [...new Set(symbols.map(symbol => symbol.file))].length,
        symbolCount: symbols.length,
        kindCounts: countBy(symbols, 'kind'),
        languageCounts: countBy(symbols, 'language'),
        symbols: symbols.sort((a, b) => a.file.localeCompare(b.file) || a.line - b.line || a.name.localeCompare(b.name))
    };
}

function formatSymbolIndex(index: any, options: any = {}) {
    const limit = Number.isFinite(options.limit) ? options.limit : 80;
    const shown = index.symbols.slice(0, limit);
    const lines = [];

    lines.push('# Symbol Index');
    lines.push('');
    lines.push(`Root: ${index.root}`);
    lines.push(`Source files scanned: ${index.fileCount}`);
    lines.push(`Files with symbols: ${index.indexedFiles}`);
    lines.push(`Symbols found: ${index.symbolCount}`);

    lines.push('');
    lines.push('## By Kind');
    lines.push(index.kindCounts.length
        ? index.kindCounts.map(item => `- ${item.name}: ${item.count}`).join('\n')
        : '- (none)');

    lines.push('');
    lines.push('## By Language');
    lines.push(index.languageCounts.length
        ? index.languageCounts.map(item => `- ${item.name}: ${item.count}`).join('\n')
        : '- (none)');

    lines.push('');
    lines.push(`## Symbols${index.symbolCount > shown.length ? ` (first ${shown.length})` : ''}`);
    if (shown.length === 0) {
        lines.push('- (none)');
    } else {
        shown.forEach(symbol => {
            lines.push(`- ${symbol.kind} ${symbol.name} (${symbol.file}:${symbol.line})`);
        });
    }

    return lines.join('\n');
}

const _helpers = {
    walkSourceFiles,
    indexFileSymbols,
    indexJavaScriptLike,
    indexPython,
    indexRust
};

export { buildSymbolIndex,
    formatSymbolIndex,
    _helpers
};
