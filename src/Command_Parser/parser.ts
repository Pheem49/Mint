export function parseCommand(aiResponse: any): any {
    let action = { type: 'none', target: '' };
    let responseText = '';
    let timestamp = null;
    let providerInfo = null;

    if (typeof aiResponse === 'string') {
        // Attempt to parse string to JSON
        try {
            const parsed = JSON.parse(aiResponse);
            action = parsed.action || action;
            responseText = parsed.response || '';
            timestamp = parsed.timestamp || null;
            providerInfo = parsed.providerInfo || null;
        } catch (e) {
            // Fallback for markdown
            const jsonMatch = aiResponse.match(/```json\n([\s\S]*?)\n```/) || aiResponse.match(/\{[\s\S]*\}/);
            if (jsonMatch) {
                try {
                    const parsed = JSON.parse(jsonMatch[jsonMatch.length > 1 ? 1 : 0]);
                    action = parsed.action || action;
                    responseText = parsed.response || '';
                    timestamp = parsed.timestamp || null;
                    providerInfo = parsed.providerInfo || null;
                } catch (err) {
                    responseText = aiResponse;
                }
            } else {
                responseText = aiResponse;
            }
        }
    } else if (typeof aiResponse === 'object') {
        action = aiResponse.action || action;
        responseText = aiResponse.response || '';
        timestamp = aiResponse.timestamp || null;
        providerInfo = aiResponse.providerInfo || null;
    }

    const parsedResponse: any = { response: responseText, action };
    if (timestamp) parsedResponse.timestamp = timestamp;
    if (providerInfo) parsedResponse.providerInfo = providerInfo;
    return parsedResponse;
}

