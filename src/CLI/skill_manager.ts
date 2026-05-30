import * as fs from 'fs';
import * as path from 'path';
import * as memoryStore from '../AI_Brain/memory_store';

/**
 * Reads a local .md or .txt file and stores it as a persistent Mint skill.
 *
 * @param filePath  Path relative to process.cwd() or absolute.
 * @returns         The stored skill record from memoryStore.
 * @throws {Error}   If the file doesn't exist, isn't a file, has the wrong extension,
 *                   or exceeds the size limit.
 */
export function learnSkillFile(filePath: string): any {
    const targetPath = path.resolve(process.cwd(), filePath);

    if (!fs.existsSync(targetPath)) {
        throw new Error(`File not found: ${targetPath}`);
    }

    const stat = fs.statSync(targetPath);
    if (!stat.isFile()) {
        throw new Error(`Path is not a file: ${targetPath}`);
    }

    const ext = path.extname(targetPath).toLowerCase();
    if (ext !== '.md' && ext !== '.txt') {
        throw new Error('Mint learn currently supports .md and .txt files only.');
    }

    const maxBytes = 256 * 1024;
    if (stat.size > maxBytes) {
        throw new Error(`File is too large (${stat.size} bytes). Limit is ${maxBytes} bytes.`);
    }

    const content = fs.readFileSync(targetPath, 'utf8');
    return memoryStore.addLearnedSkill(path.basename(targetPath), targetPath, content);
}
