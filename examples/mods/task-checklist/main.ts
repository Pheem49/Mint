export function register(mint: any) {
  const render = () => {
    const tasks: string[] = mint.state.get('tasks') || [];
    mint.ui.set('side_panel', {type:'stack', children:[
      {type:'text', text:`${tasks.length} tasks`},
      {type:'table', columns:['Task'], rows:tasks.map(task => [task])},
      {type:'input', label:'New task', action:'add'},
      {type:'button', label:'Clear tasks', action:'clear'}
    ]});
  };
  mint.commands.register('add', (value: string) => {
    if (value?.trim()) mint.state.set('tasks', [...(mint.state.get('tasks') || []), value.trim()]);
    render();
  }, 'Add a task');
  mint.commands.register('clear', () => {mint.state.set('tasks', []); render();}, 'Clear the checklist');
  render();
}
