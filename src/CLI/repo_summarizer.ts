import * as fs from 'fs'
import * as path from 'path'
import { execFileSync  } from 'child_process'

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
    '.css': 'CSS',
    '.html': 'HTML',
    '.js': 'JavaScript',
    '.json': 'JSON',
    '.jsx': 'JavaScript',
    '.md': 'Markdown',
    '.mjs': 'JavaScript',
    '.py': 'Python',
    '.rs': 'Rust',
    '.sh': 'Shell',
    '.ts': 'TypeScript',
    '.tsx': 'TypeScript',
    '.vue': 'Vue',
    '.yaml': 'YAML',
    '.yml': 'YAML'
};

function safeReadJson(filePath) {
    try {
        return JSON.parse(fs.readFileSync(filePath, 'utf8'));
    } catch (_) {
        return null;
    }
}

function safeGit(root, args) {
    try {
        return execFileSync('git', args, {
            cwd: root,
            encoding: 'utf8',
            stdio: ['ignore', 'pipe', 'ignore'],
            maxBuffer: 1024 * 1024
        }).trim();
    } catch (_) {
        return '';
    }
}

function walkFiles(root: string, options: any = {}) {
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

            if (entry.isFile()) {
                files.push(relativePath);
            }
        }
    }

    visit(root);
    return files;
}

function summarizeLanguages(files) {
    const counts = new Map();
    for (const file of files) {
        const language = LANGUAGE_BY_EXT[path.extname(file).toLowerCase()] || 'Other';
        counts.set(language, (counts.get(language) || 0) + 1);
    }

    return Array.from(counts.entries())
        .map(([language, count]) => ({ language, count }))
        .sort((a, b) => b.count - a.count || a.language.localeCompare(b.language));
}

function findImportantFiles(files) {
    const importantPatterns = [
        /^README/i,
        /^package\.json$/,
        /^RELEASE_NOTES\.md$/,
        /^CHANGELOG/i,
        /^Dockerfile$/,
        /^docker-compose\./,
        /^\.github\//,
        /^src\//,
        /^tests?\//,
        /config/i,
        /\.(test|spec)\.[cm]?[jt]sx?$/
    ];

    return files
        .filter(file => importantPatterns.some(pattern => pattern.test(file)))
        .slice(0, 30);
}

function summarizeTopDirs(files) {
    const counts = new Map();
    for (const file of files) {
        const top = file.includes(path.sep) ? file.split(path.sep)[0] : '(root)';
        counts.set(top, (counts.get(top) || 0) + 1);
    }

    return Array.from(counts.entries())
        .map(([dir, count]) => ({ dir, count }))
        .sort((a, b) => b.count - a.count || a.dir.localeCompare(b.dir))
        .slice(0, 12);
}

function summarizePackage(root) {
    const pkg = safeReadJson(path.join(root, 'package.json'));
    if (!pkg) return null;

    return {
        name: pkg.name || '',
        version: pkg.version || '',
        description: pkg.description || '',
        scripts: Object.keys(pkg.scripts || {}),
        dependencies: Object.keys(pkg.dependencies || {}),
        devDependencies: Object.keys(pkg.devDependencies || {})
    };
}

function summarizeGit(root) {
    const inside = safeGit(root, ['rev-parse', '--is-inside-work-tree']) === 'true';
    if (!inside) {
        return { isRepo: false };
    }

    return {
        isRepo: true,
        branch: safeGit(root, ['branch', '--show-current']) || '(detached HEAD)',
        status: safeGit(root, ['status', '--short']) || '(clean)',
        diffStat: safeGit(root, ['diff', '--stat']) || '(no unstaged diff)'
    };
}

function summarizeRepository(targetPath = process.cwd(), options: any = {}) {
    const root = path.resolve(targetPath);
    const stat = fs.statSync(root);
    if (!stat.isDirectory()) {
        throw new Error(`Repository path is not a directory: ${root}`);
    }

    const files = walkFiles(root, options);
    return {
        root,
        fileCount: files.length,
        topDirs: summarizeTopDirs(files),
        languages: summarizeLanguages(files),
        importantFiles: findImportantFiles(files),
        package: summarizePackage(root),
        git: summarizeGit(root)
    };
}

function formatList(items, formatter, emptyText = '(none)') {
    if (!Array.isArray(items) || items.length === 0) return `- ${emptyText}`;
    return items.map(formatter).join('\n');
}

function formatMultilineField(label, value, emptyText) {
    const text = value || emptyText;
    const lines = String(text).split('\n');
    if (lines.length === 1) {
        return `- ${label}: ${lines[0]}`;
    }
    return [
        `- ${label}:`,
        ...lines.map(line => `  ${line}`)
    ].join('\n');
}

function formatRepoSummary(summary) {
    const pkg = summary.package;
    const git = summary.git;
    const lines = [];

    lines.push(`# Repository Summary`);
    lines.push('');
    lines.push(`Root: ${summary.root}`);
    if (pkg?.name) {
        lines.push(`Package: ${pkg.name}${pkg.version ? ` v${pkg.version}` : ''}`);
    }
    if (pkg?.description) {
        lines.push(`Description: ${pkg.description}`);
        lines.push('');
    }
    lines.push(`Files scanned: ${summary.fileCount}`);

    lines.push('');
    lines.push(`## Git`);
    if (!git?.isRepo) {
        lines.push('- Not a git repository');
    } else {
        lines.push(`- Branch: ${git.branch}`);
        lines.push(formatMultilineField('Status', git.status, '(clean)'));
        lines.push(formatMultilineField('Diff', git.diffStat, '(no unstaged diff)'));
    }

    lines.push('');
    lines.push(`## Top Directories`);
    lines.push(formatList(
        summary.topDirs,
        item => `- ${item.dir}: ${item.count} file(s)`
    ));

    lines.push('');
    lines.push(`## Languages`);
    lines.push(formatList(
        summary.languages.slice(0, 10),
        item => `- ${item.language}: ${item.count} file(s)`
    ));

    lines.push('');
    lines.push(`## Package Scripts`);
    lines.push(formatList(
        pkg?.scripts || [],
        script => `- ${script}`
    ));

    lines.push('');
    lines.push(`## Dependencies`);
    if (pkg) {
        const depCount = pkg.dependencies.length;
        const devDepCount = pkg.devDependencies.length;
        lines.push(`- Runtime: ${depCount}`);
        lines.push(`- Development: ${devDepCount}`);
        const notable = pkg.dependencies.slice(0, 12);
        if (notable.length > 0) {
            lines.push(`- Notable runtime deps: ${notable.join(', ')}`);
        }
    } else {
        lines.push('- No package.json found');
    }

    lines.push('');
    lines.push(`## Important Files`);
    lines.push(formatList(
        summary.importantFiles,
        file => `- ${file}`
    ));

    return lines.join('\n');
}

const _helpers = {
    walkFiles,
    summarizeLanguages,
    summarizeTopDirs,
    findImportantFiles
};

export { summarizeRepository,
    formatRepoSummary,
    _helpers
};
