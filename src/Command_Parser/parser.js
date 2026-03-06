function parseCommand(aiResponse) {
    let action = { type: 'none', target: '' };
    let responseText = '';

    if (typeof aiResponse === 'string') {
        // Attempt to parse string to JSON
        try {
            const parsed = JSON.parse(aiResponse);
            action = parsed.action || action;
            responseText = parsed.response || '';
        } catch (e) {
            // Fallback for markdown
            const jsonMatch = aiResponse.match(/```json\n([\s\S]*?)\n```/) || aiResponse.match(/\{[\s\S]*\}/);
            if (jsonMatch) {
                try {
                    const parsed = JSON.parse(jsonMatch[jsonMatch.length > 1 ? 1 : 0]);
                    action = parsed.action || action;
                    responseText = parsed.response || '';
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
    }

    return { response: responseText, action };
}

module.exports = { parseCommand };
