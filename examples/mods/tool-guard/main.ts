export function register(mint: any) {
  mint.ui.set('above_composer', {type:'badge', text:'Tool Guard · recursive deletion blocked'});
  mint.on('pre_tool_use', (event: any) => {
    if (event.action === 'run_shell' && /\brm\s+(-[^\s]*r[^\s]*\s+|--recursive\b)/.test(event.input.command || '')) {
      mint.log(`Blocked call ${event.callId}`);
      return {block:'Recursive deletion requires disabling Tool Guard first.'};
    }
  });
}
