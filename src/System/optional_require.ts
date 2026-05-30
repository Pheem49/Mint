
/**
 * Helper: ลอง require package แบบ dynamic
 * ถ้าหาไม่เจอให้ throw Error พร้อม install guide
 */
export function requireOptional(pkg: string, installHint?: string): any {
    try {
        return require(pkg)
    } catch (e: any) {
        if (e.code === 'MODULE_NOT_FOUND') {
            const hint = installHint || `npm install ${pkg}`
            throw new Error(
                `[Mint] Optional package "${pkg}" is not installed.\n` +
                `To use this feature, run: ${hint}\n` +
                `(This package is not bundled by default to keep Mint lightweight.)`
            )
        }
        throw e
    }
}
