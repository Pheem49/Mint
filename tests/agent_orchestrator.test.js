/**
 * Tests: agent_orchestrator.js
 */

const orchestrator = require('../src/AI_Brain/agent_orchestrator');

describe('Agent Orchestrator', () => {
    beforeEach(() => {
        orchestrator.resetAgent();
    });

    test('starts with general agent', () => {
        const agent = orchestrator.getCurrentAgent();
        expect(agent.name).toBe('Mint Default');
    });

    test('can switch to coder agent', () => {
        orchestrator.setAgent('coder');
        const agent = orchestrator.getCurrentAgent();
        expect(agent.name).toBe('Mint Coder');
    });

    test('can switch to researcher agent', () => {
        orchestrator.setAgent('researcher');
        const agent = orchestrator.getCurrentAgent();
        expect(agent.name).toBe('Mint Researcher');
    });

    test('falls back to general for invalid agent', () => {
        orchestrator.setAgent('nonexistent');
        const agent = orchestrator.getCurrentAgent();
        expect(agent.name).toBe('Mint Default');
    });

    test('lists available agents', () => {
        const agents = orchestrator.listAgents();
        expect(agents).toContain('general');
        expect(agents).toContain('coder');
        expect(agents).toContain('researcher');
    });
});
