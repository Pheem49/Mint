import { execFile  } from 'child_process'

const plugin = {
    name: 'docker',
    description: 'Interacts with local Docker engine. Valid targets include: "start <container>", "stop <container>", "restart <container>", "list".',
    
    async execute(target: any) {
        return new Promise((resolve) => {
            console.log(`[Docker Plugin] Executing command: ${target }`);
            
            const rawTarget = (target || '').trim();
            const [rawAction, ...args] = rawTarget.split(/\s+/);
            const action = (rawAction || '').toLowerCase();
            const containerName = args.join(' ');
            let commandArgs: string[] = [];
            
            if (action === 'list') {
                commandArgs = ['ps', '--format', '{{.Names}} ({{.Status}})'];
            } else if (['start', 'stop', 'restart'].includes(action) && containerName) {
                commandArgs = [action, containerName];
            } else {
                return resolve(`Invalid docker command or missing container name: ${target}`);
            }

            execFile('docker', commandArgs, (error: any, stdout, stderr) => {
                if (error) {
                    const stderrText = stderr || '';
                    if (error.code === 127 || stderrText.includes('not found') || error.code === 'ENOENT') {
                        return resolve('Error: Docker is not installed or not in PATH.');
                    }
                    if (stderrText.toLowerCase().includes('permission denied')) {
                        return resolve('Error: Permission denied. You might need to add your user to the "docker" group.');
                    }
                    return resolve(`Docker Error: ${stderrText || error.message}`);
                }

                if (action === 'list') {
                    const containers = stdout.trim();
                    if (!containers) return resolve("No running Docker containers found.");
                    return resolve(`Running Containers:\n${containers}`);
                }

                resolve(`Successfully executed "docker ${action}" on container "${containerName}".`);
            });
        });
    }
};

export = plugin;
