import * as http from 'http'
import { execFile  } from 'child_process'
import * as crypto from 'crypto'
import axios from 'axios'
import { readConfig, writeConfig  } from '../System/config_manager'

const TOKEN_URL = 'https://oauth2.googleapis.com/token';
const AUTH_URL = 'https://accounts.google.com/o/oauth2/v2/auth';
const DEFAULT_SCOPES = [
    'https://www.googleapis.com/auth/gmail.readonly',
    'https://www.googleapis.com/auth/gmail.compose'
];

function buildRedirectUri(port) {
    return `http://127.0.0.1:${port}/oauth2callback`;
}

function buildAuthUrl({ clientId, redirectUri, state, scopes = DEFAULT_SCOPES }) {
    const params = new URLSearchParams({
        client_id: clientId,
        redirect_uri: redirectUri,
        response_type: 'code',
        scope: scopes.join(' '),
        access_type: 'offline',
        prompt: 'consent',
        state
    });

    return `${AUTH_URL}?${params.toString()}`;
}

function openBrowser(url) {
    const command = process.platform === 'darwin'
        ? 'open'
        : process.platform === 'win32'
            ? 'cmd'
            : 'xdg-open';
    const args = process.platform === 'win32' ? ['/c', 'start', '', url] : [url];

    return new Promise<void>((resolve, reject) => {
        execFile(command, args, (error) => {
            if (error) {
                reject(error);
                return;
            }
            resolve();
        });
    });
}

async function exchangeCodeForToken({ clientId, clientSecret, code, redirectUri }) {
    const params = new URLSearchParams({
        client_id: clientId,
        client_secret: clientSecret,
        code,
        redirect_uri: redirectUri,
        grant_type: 'authorization_code'
    });

    const response = await axios.post(TOKEN_URL, params.toString(), {
        headers: { 'Content-Type': 'application/x-www-form-urlencoded' }
    });

    return response.data;
}

function waitForOAuthCode({ port = 0, state, timeoutMs = 180000 }) {
    return new Promise((resolve, reject) => {
        let settled = false;
        let timer = null;

        const finish = (error: any, value?: any) => {
            if (settled) return;
            settled = true;
            if (timer) clearTimeout(timer);
            server.close(() => {
                if (error) reject(error);
                else resolve(value);
            });
        };

        const server = http.createServer((req, res) => {
            try {
                const url = new URL(req.url, 'http://127.0.0.1');
                if (url.pathname !== '/oauth2callback') {
                    res.writeHead(404, { 'Content-Type': 'text/plain' });
                    res.end('Not found');
                    return;
                }

                const returnedState = url.searchParams.get('state');
                const error = url.searchParams.get('error');
                const code = url.searchParams.get('code');

                if (error) {
                    res.writeHead(400, { 'Content-Type': 'text/plain' });
                    res.end(`Gmail authorization failed: ${error}`);
                    finish(new Error(`Gmail authorization failed: ${error}`));
                    return;
                }

                if (!code || returnedState !== state) {
                    res.writeHead(400, { 'Content-Type': 'text/plain' });
                    res.end('Invalid Gmail authorization response.');
                    finish(new Error('Invalid Gmail authorization response.'));
                    return;
                }

                res.writeHead(200, { 'Content-Type': 'text/html; charset=utf-8' });
                res.end('<h1>Gmail connected</h1><p>You can close this window and return to Mint.</p>');
                finish(null, code);
            } catch (err) {
                res.writeHead(500, { 'Content-Type': 'text/plain' });
                res.end('Internal error.');
                finish(err);
            }
        });

        server.on('error', finish);
        server.listen(port, '127.0.0.1', () => {
            timer = setTimeout(() => {
                finish(new Error('Timed out waiting for Gmail authorization callback.'));
            }, timeoutMs);
        });
    });
}

async function runGmailAuth(options: any = {}) {
    const logger = options.logger || console;
    const config = options.readConfig ? options.readConfig() : readConfig();
    const clientId = (config.gmailClientId || '').trim();
    const clientSecret = (config.gmailClientSecret || '').trim();
    const userId = (config.gmailUserId || 'me').trim() || 'me';

    if (!clientId || !clientSecret) {
        throw new Error('Missing Gmail OAuth Client ID or Client Secret. Run `mint onboard` and fill Gmail API credentials first.');
    }

    const state = crypto.randomBytes(16).toString('hex');
    const actualPort = options.getAuthorizationCode ? Number(options.port || 8787) : await reserveLocalPort(Number(options.port || 0));
    const redirectUri = buildRedirectUri(actualPort);
    const codePromise = options.getAuthorizationCode
        ? null
        : waitForOAuthCode({
            port: actualPort,
            state,
            timeoutMs: options.timeoutMs || 180000
        });
    const authUrl = buildAuthUrl({ clientId, redirectUri, state, scopes: options.scopes || DEFAULT_SCOPES });

    logger.log(`Open this Google OAuth consent link for Gmail (${userId}):\n${authUrl}\n`);

    if (options.openBrowser !== false) {
        const browserOpener = options.openBrowser || openBrowser;
        await browserOpener(authUrl);
    }

    const code = options.getAuthorizationCode
        ? await options.getAuthorizationCode({ authUrl, state, redirectUri })
        : await codePromise;
    const token = await exchangeCodeForToken({
        clientId,
        clientSecret,
        code,
        redirectUri
    });

    if (!token.refresh_token) {
        throw new Error('Google did not return a refresh token. Re-run `mint gmail auth`; the flow uses prompt=consent to request one.');
    }

    const nextConfig = {
        ...config,
        gmailRefreshToken: token.refresh_token,
        gmailUserId: userId,
        pluginGmailEnabled: true
    };

    const writeResult = options.writeConfig ? options.writeConfig(nextConfig) : writeConfig(nextConfig);
    if (writeResult && writeResult.success === false) {
        throw new Error(writeResult.message || 'Failed to save Gmail refresh token.');
    }

    return {
        success: true,
        userId,
        scopes: options.scopes || DEFAULT_SCOPES
    };
}

function reserveLocalPort(port = 0): Promise<number> {
    return new Promise<number>((resolve, reject) => {
        const server = http.createServer();
        server.on('error', reject);
        server.listen(port, '127.0.0.1', () => {
            const addr = server.address();
            const actualPort = (addr && typeof addr === 'object') ? addr.port : 0;
            server.close(() => resolve(actualPort));
        });
    });
}

export { DEFAULT_SCOPES,
    buildRedirectUri,
    buildAuthUrl,
    exchangeCodeForToken,
    waitForOAuthCode,
    reserveLocalPort,
    runGmailAuth
 }
