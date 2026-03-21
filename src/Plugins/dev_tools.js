const { exec } = require('child_process');

function execPromise(command, cwd) {
    return new Promise((resolve) => {
        exec(command, { cwd }, (error, stdout, stderr) => {
            if (error) {
                resolve(`Error: ${stderr || error.message}`);
                return;
            }
            resolve(stdout.trim());
        });
    });
}

module.exports = {
    name: 'dev_tools',
    description: 'Get git status, recent commits, or branch information for a project. Instruction MUST be "git status", "git log", or "git branch".',
    
    async execute(instruction) {
        let cwd = process.cwd();
        let cmd = (instruction || '').toLowerCase();
        
        let gitCmd = '';
        if (cmd.includes('status')) {
            gitCmd = 'git status -s';
        } else if (cmd.includes('log') || cmd.includes('commit')) {
            gitCmd = 'git log -n 5 --oneline';
        } else if (cmd.includes('branch')) {
            gitCmd = 'git branch';
        } else {
            return "ไม่เข้าใจคำสั่ง git ค่ะ ระบุเป็น status, log, หรือ branch นะคะ (ตัวอย่าง: git status)";
        }

        const output = await execPromise(gitCmd, cwd);
        if (!output || output.startsWith('Error:')) {
            return `ไม่สามารถดึงข้อมูล Git ได้ค่ะ: ${output}`;
        }

        return `ผลลัพธ์จาก Git:\n${output}`;
    }
};
