const fs = require('fs');
const os = require('os');
const path = require('path');

describe('task_manager persistent task engine', () => {
    let tempHome;
    let taskManager;

    beforeEach(() => {
        jest.resetModules();
        tempHome = fs.mkdtempSync(path.join(os.tmpdir(), 'mint-task-home-'));
        jest.doMock('os', () => ({
            ...jest.requireActual('os'),
            homedir: () => tempHome
        }));
        taskManager = require('../dist/src/System/task_manager');
    });

    afterEach(() => {
        jest.dontMock('os');
        fs.rmSync(tempHome, { recursive: true, force: true });
    });

    test('stores checkpoints, subtasks, and artifacts durably', () => {
        const task = taskManager.addTask('ship feature');
        taskManager.addSubtask(task.id, 'inspect repo');
        taskManager.addCheckpoint(task.id, { phase: 'inspect', message: 'read files' });
        taskManager.addArtifact(task.id, { type: 'file', path: 'notes.md' });

        const saved = taskManager.getTask(task.id);
        expect(saved.subtasks).toHaveLength(1);
        expect(saved.checkpoints).toHaveLength(1);
        expect(saved.steps).toHaveLength(1);
        expect(saved.artifacts[0].path).toBe('notes.md');
        expect(saved.lastCheckpointAt).toBeTruthy();
    });

    test('requeues interrupted running tasks for resume', () => {
        const task = taskManager.addTask('long task');
        taskManager.updateTask(task.id, { status: 'running' });

        const resumed = taskManager.resumeRunningTasks();
        const saved = taskManager.getTask(task.id);

        expect(resumed.map(item => item.id)).toContain(task.id);
        expect(saved.status).toBe('pending');
        expect(saved.checkpoints.some(item => item.phase === 'resume_after_restart')).toBe(true);
    });

    test('schedules retry before final failure', () => {
        const task = taskManager.addTask('retry task');
        taskManager.failTaskWithRetry(task.id, 'first failure');
        expect(taskManager.getTask(task.id).status).toBe('pending');

        taskManager.failTaskWithRetry(task.id, 'second failure');
        const saved = taskManager.getTask(task.id);
        expect(saved.status).toBe('failed');
        expect(saved.retryCount).toBe(2);
    });
});
