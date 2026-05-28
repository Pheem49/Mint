const { buildApprovalRequest, executeApprovedAction } = require('../src/System/ipc_handlers');

describe('IPC action approval flow', () => {
    test('does not request approval for safe actions', () => {
        expect(buildApprovalRequest({ type: 'open_url', target: 'https://example.com' })).toBeNull();
    });

    test('requests approval for approval-tier actions', () => {
        const approval = buildApprovalRequest({ type: 'system_automation', target: 'volume:50' });

        expect(approval).toMatchObject({
            required: true,
            tier: 'approval',
            reason: 'system automation requires approval',
            action: { type: 'system_automation', target: 'volume:50' }
        });
    });

    test('requests dangerous approval for dangerous actions', () => {
        const approval = buildApprovalRequest({ type: 'delete_file', target: 'notes.txt' });

        expect(approval).toMatchObject({
            required: true,
            tier: 'dangerous',
            action: { type: 'delete_file', target: 'notes.txt' }
        });
    });

    test('executes approval-tier actions with approval flag', async () => {
        const executeAction = jest.fn(async () => 'volume changed');
        const action = { type: 'system_automation', target: 'volume:50' };

        const result = await executeApprovedAction(executeAction, action, {});

        expect(executeAction).toHaveBeenCalledWith(action, expect.objectContaining({
            allowApproval: true,
            allowDangerous: false,
            source: 'user_approved_action'
        }));
        expect(result).toMatchObject({
            success: true,
            tier: 'approval',
            message: 'volume changed'
        });
    });

    test('executes dangerous actions with dangerous flag', async () => {
        const executeAction = jest.fn(async () => undefined);
        const action = { type: 'delete_file', target: 'notes.txt' };

        const result = await executeApprovedAction(executeAction, action, {});

        expect(executeAction).toHaveBeenCalledWith(action, expect.objectContaining({
            allowApproval: false,
            allowDangerous: true,
            source: 'user_approved_action'
        }));
        expect(result).toMatchObject({
            success: true,
            tier: 'dangerous',
            message: 'Action completed.'
        });
    });
});
