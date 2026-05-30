const axios = require('axios');
const { GoogleGenAI } = require('@google/genai');
const { getAvailableProviders } = require('../System/config_manager');

const DEFAULT_GEMINI_MODEL = 'gemini-2.5-flash';
const ALL_PROVIDERS = ['anthropic', 'openai', 'gemini', 'local_openai', 'ollama', 'huggingface'];

function splitDataUri(dataUri = '') {
    const match = String(dataUri).match(/^data:([^;]+);base64,([\s\S]+)$/);
    if (!match) return null;
    return {
        mimeType: match[1],
        data: match[2]
    };
}

function contentToText(content) {
    if (content && typeof content === 'object' && !Array.isArray(content)) {
        return String(content.text || '');
    }
    return String(content || '');
}

function contentToGeminiParts(content) {
    const text = contentToText(content);
    const parts = text ? [{ text }] : [];
    if (content && typeof content === 'object') {
        const images = Array.isArray(content.imageDataUris)
            ? content.imageDataUris
            : (content.imageDataUri ? [content.imageDataUri] : []);
        for (const item of images) {
            const image = splitDataUri(item);
            if (image) parts.push({ inlineData: { mimeType: image.mimeType, data: image.data } });
        }
        if (content.audioDataUri) {
            const audio = splitDataUri(content.audioDataUri);
            if (audio) parts.push({ inlineData: { mimeType: audio.mimeType, data: audio.data } });
        }
    }
    return parts.length > 0 ? parts : [{ text: '' }];
}

function contentToOpenAIContent(content) {
    const text = contentToText(content) || 'Analyze this input.';
    if (content && typeof content === 'object') {
        const images = Array.isArray(content.imageDataUris)
            ? content.imageDataUris
            : (content.imageDataUri ? [content.imageDataUri] : []);
        if (images.length > 0) {
            return [
                { type: 'text', text },
                ...images.map(item => ({ type: 'image_url', image_url: { url: item } }))
            ];
        }
    }
    return text;
}

function contentToAnthropicContent(content) {
    const text = contentToText(content) || 'Analyze this input.';
    if (content && typeof content === 'object') {
        const images = Array.isArray(content.imageDataUris)
            ? content.imageDataUris
            : (content.imageDataUri ? [content.imageDataUri] : []);
        if (images.length > 0) {
            const blocks = [];
            for (const item of images) {
                const image = splitDataUri(item);
                if (image) {
                    blocks.push({ type: 'image', source: { type: 'base64', media_type: image.mimeType, data: image.data } });
                }
            }
            blocks.push({ type: 'text', text });
            return blocks;
        }
    }
    return text;
}

function contentToOllamaMessage(content) {
    const text = contentToText(content) || 'Analyze this input.';
    const message = { role: 'user', content: text };
    if (content && typeof content === 'object') {
        const images = Array.isArray(content.imageDataUris)
            ? content.imageDataUris
            : (content.imageDataUri ? [content.imageDataUri] : []);
        const imagePayloads = images
            .map(item => splitDataUri(item))
            .filter(Boolean)
            .map(image => image.data);
        if (imagePayloads.length > 0) message.images = imagePayloads;
    }
    return message;
}

function getProviderAttemptOrder(config = {}, options = {}) {
    const supported = options.supported || ALL_PROVIDERS;
    const available = (options.availableProviders || getAvailableProviders(config))
        .filter(provider => supported.includes(provider));
    const requested = options.requested || config.aiProvider || 'gemini';
    const priority = (options.priority || ALL_PROVIDERS).filter(provider => supported.includes(provider));
    const ordered = [];

    if (supported.includes(requested) && available.includes(requested)) {
        ordered.push(requested);
    }

    for (const provider of priority) {
        if (available.includes(provider) && !ordered.includes(provider)) {
            ordered.push(provider);
        }
    }

    return ordered.length > 0 ? ordered : ['gemini'];
}

function getProviderModel(provider, config = {}) {
    switch (provider) {
        case 'anthropic':
            return config.anthropicModel || 'claude-3-5-sonnet-latest';
        case 'openai':
            return config.openaiModel || 'gpt-4o';
        case 'local_openai':
            return config.localModelName || 'local-model';
        case 'ollama':
            return config.ollamaModel || 'llama3:latest';
        case 'huggingface':
            return config.hfModel || 'meta-llama/Meta-Llama-3-8B-Instruct';
        case 'gemini':
        default:
            return config.geminiModel || DEFAULT_GEMINI_MODEL;
    }
}

class AgentProviderClient {
    constructor(options = {}) {
        this.provider = options.provider || 'gemini';
        this.providerOrder = options.providerOrder && options.providerOrder.length
            ? options.providerOrder
            : [this.provider];
        this.config = options.config || {};
        this.history = options.history || [];
        this.systemInstruction = options.systemInstruction || '';
        this.responseMimeType = options.responseMimeType || 'application/json';
        this.maxTokens = options.maxTokens || 8192;
        this.lastSuccessfulProvider = null;
        this.usageTotals = {};
    }

    recordUsage(provider, model, usage = {}) {
        const key = `${provider}:${model || ''}`;
        if (!this.usageTotals[key]) {
            this.usageTotals[key] = {
                provider,
                model,
                requests: 0,
                inputTokens: 0,
                cacheReads: 0,
                outputTokens: 0,
                reasoningTokens: 0,
                totalTokens: 0
            };
        }

        const row = this.usageTotals[key];
        row.requests += 1;
        row.inputTokens += Number(usage.inputTokens) || 0;
        row.cacheReads += Number(usage.cacheReads) || 0;
        row.outputTokens += Number(usage.outputTokens) || 0;
        row.reasoningTokens += Number(usage.reasoningTokens) || 0;
        row.totalTokens += Number(usage.totalTokens) || 0;
    }

    getUsageSummary() {
        return Object.values(this.usageTotals);
    }

    async sendMessage(observation) {
        this.history.push({ role: 'user', content: observation });

        const failures = [];
        for (const provider of this.providerOrder) {
            this.provider = provider;
            try {
                let responseText = '';
                if (provider === 'anthropic') responseText = await this.callAnthropic();
                else if (provider === 'openai' || provider === 'local_openai') responseText = await this.callOpenAI();
                else if (provider === 'ollama') responseText = await this.callOllama();
                else if (provider === 'huggingface') responseText = await this.callHuggingFace();
                else responseText = await this.callGemini();

                this.history.push({ role: 'assistant', content: responseText });
                this.lastSuccessfulProvider = provider;
                return responseText;
            } catch (error) {
                const message = error.message || error.code || 'unknown error';
                failures.push(`${provider}: ${message}`);
                if (process.env.MINT_DEBUG === '1') {
                    console.error(`[ProviderAdapter] Provider '${provider}' failed: ${message}`);
                }
            }
        }

        throw new Error(`All providers failed. ${failures.join(' | ')}`);
    }

    async callAnthropic() {
        const apiKey = this.config.anthropicApiKey || process.env.ANTHROPIC_API_KEY;
        const model = getProviderModel('anthropic', this.config);
        const messages = this.history.map(m => ({
            role: m.role,
            content: contentToAnthropicContent(m.content)
        }));

        const response = await axios.post('https://api.anthropic.com/v1/messages', {
            model,
            max_tokens: this.maxTokens,
            system: this.systemInstruction,
            messages
        }, {
            headers: {
                'x-api-key': apiKey,
                'anthropic-version': '2023-06-01',
                'content-type': 'application/json'
            }
        });
        const usage = response.data.usage || {};
        this.recordUsage('anthropic', model, {
            inputTokens: usage.input_tokens,
            cacheReads: usage.cache_read_input_tokens,
            outputTokens: usage.output_tokens,
            totalTokens: (Number(usage.input_tokens) || 0) + (Number(usage.output_tokens) || 0)
        });
        return response.data.content[0].text;
    }

    async callOpenAI() {
        const isLocal = this.provider === 'local_openai';
        const apiKey = isLocal ? 'not-needed' : (this.config.openaiApiKey || process.env.OPENAI_API_KEY);
        const baseUrl = isLocal ? (this.config.localApiBaseUrl || 'http://localhost:1234/v1') : 'https://api.openai.com/v1';
        const model = getProviderModel(this.provider, this.config);
        const messages = [
            { role: 'system', content: this.systemInstruction },
            ...this.history.map(m => ({
                role: m.role,
                content: contentToOpenAIContent(m.content)
            }))
        ];

        const response = await axios.post(`${baseUrl.replace(/\/$/, '')}/chat/completions`, {
            model,
            messages,
            response_format: isLocal ? undefined : { type: 'json_object' }
        }, {
            headers: {
                'Authorization': `Bearer ${apiKey}`,
                'Content-Type': 'application/json'
            }
        });
        const usage = response.data.usage || {};
        this.recordUsage(this.provider, model, {
            inputTokens: usage.prompt_tokens,
            cacheReads: usage.prompt_tokens_details && usage.prompt_tokens_details.cached_tokens,
            outputTokens: usage.completion_tokens,
            reasoningTokens: usage.completion_tokens_details && usage.completion_tokens_details.reasoning_tokens,
            totalTokens: usage.total_tokens
        });
        return response.data.choices[0].message.content;
    }

    async callGemini() {
        const apiKey = this.config.apiKey || process.env.GEMINI_API_KEY;
        const model = getProviderModel('gemini', this.config);
        const ai = new GoogleGenAI({ apiKey });
        const recentHistory = this.history.slice(-16);
        const priorHistory = recentHistory.slice(0, -1);
        const lastEntry = recentHistory[recentHistory.length - 1] || { content: '' };
        const history = priorHistory.map(m => ({
            role: m.role === 'assistant' ? 'model' : 'user',
            parts: contentToGeminiParts(m.content)
        }));
        const chat = ai.chats.create({
            model,
            config: {
                systemInstruction: this.systemInstruction,
                responseMimeType: this.responseMimeType
            },
            history
        });

        const response = await chat.sendMessage({ message: contentToGeminiParts(lastEntry.content) });
        const usage = response.usageMetadata || {};
        this.recordUsage('gemini', model, {
            inputTokens: usage.promptTokenCount,
            cacheReads: usage.cachedContentTokenCount,
            outputTokens: usage.candidatesTokenCount,
            reasoningTokens: usage.thoughtsTokenCount,
            totalTokens: usage.totalTokenCount
        });
        return typeof response.text === 'function' ? response.text() : response.text;
    }

    async callOllama() {
        const model = getProviderModel('ollama', this.config);
        const baseUrl = (this.config.ollamaHost || 'http://localhost:11434').replace(/\/$/, '');
        const messages = [
            { role: 'system', content: this.systemInstruction },
            ...this.history.map(m => m.role === 'assistant'
                ? { role: 'assistant', content: contentToText(m.content) }
                : contentToOllamaMessage(m.content))
        ];
        const response = await axios.post(`${baseUrl}/api/chat`, {
            model,
            messages,
            format: this.responseMimeType === 'application/json' ? 'json' : undefined,
            stream: false
        });
        return response.data.message.content;
    }

    async callHuggingFace() {
        const apiKey = this.config.hfApiKey || process.env.HF_API_KEY;
        const model = getProviderModel('huggingface', this.config);
        const messages = [
            { role: 'system', content: this.systemInstruction },
            ...this.history.map(m => ({
                role: m.role,
                content: contentToOpenAIContent(m.content)
            }))
        ];
        const response = await axios.post(`https://api-inference.huggingface.co/models/${model}/v1/chat/completions`, {
            model,
            messages,
            max_tokens: this.maxTokens
        }, {
            headers: {
                'Authorization': `Bearer ${apiKey}`,
                'Content-Type': 'application/json'
            }
        });
        return response.data.choices[0].message.content;
    }
}

module.exports = {
    DEFAULT_GEMINI_MODEL,
    ALL_PROVIDERS,
    AgentProviderClient,
    getProviderAttemptOrder,
    getProviderModel,
    _helpers: {
        splitDataUri,
        contentToGeminiParts,
        contentToOpenAIContent,
        contentToAnthropicContent,
        contentToOllamaMessage
    }
};
