// Backward-compatible CLI entry point for Mint actions.
// Keep execution rules in one place so desktop, CLI, and agents share safety behavior.
import * as actionExecutor from './src/System/action_executor'
export default actionExecutor
