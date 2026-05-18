// Backward-compatible CLI entry point for Mint actions.
// Keep execution rules in one place so desktop, CLI, and agents share safety behavior.
module.exports = require('./src/System/action_executor');
