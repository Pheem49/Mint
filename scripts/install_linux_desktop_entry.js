'use strict';

const fs = require('fs');
const os = require('os');
const path = require('path');

if (process.platform !== 'linux') {
    console.log('Desktop entry installation is only needed on Linux.');
    process.exit(0);
}

const projectRoot = path.resolve(__dirname, '..');
const home = os.homedir();
const iconSource = path.join(projectRoot, 'assets', 'icon.png');
const iconTargets = [16, 24, 32, 48, 64, 128, 256, 512].map(size =>
    path.join(home, '.local', 'share', 'icons', 'hicolor', `${size}x${size}`, 'apps', 'mint-ai.png')
);
const desktopTarget = path.join(home, '.local', 'share', 'applications', 'mint-ai.desktop');

if (!fs.existsSync(iconSource)) {
    throw new Error(`Missing icon: ${iconSource}`);
}

fs.mkdirSync(path.dirname(desktopTarget), { recursive: true });
for (const iconTarget of iconTargets) {
    fs.mkdirSync(path.dirname(iconTarget), { recursive: true });
    fs.copyFileSync(iconSource, iconTarget);
}

const desktopEntry = [
    '[Desktop Entry]',
    'Type=Application',
    'Name=Mint',
    'Comment=Mint AI desktop assistant',
    `Exec=${process.execPath} ${path.join(projectRoot, 'node_modules', 'electron', 'cli.js')} ${projectRoot}`,
    'Icon=mint-ai',
    'Terminal=false',
    'Categories=Utility;',
    'StartupNotify=true',
    'StartupWMClass=Mint',
    ''
].join('\n');

fs.writeFileSync(desktopTarget, desktopEntry, 'utf8');
fs.chmodSync(desktopTarget, 0o755);

console.log(`Installed ${desktopTarget}`);
console.log(`Installed ${iconTargets.length} icon sizes as mint-ai`);
