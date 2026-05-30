const fs = require('fs');
const path = require('path');
const os = require('os');

const CONFIG_DIR = path.join(os.homedir(), '.config', 'mint');
const TASKS_FILE = path.join(CONFIG_DIR, 'tasks.json');

// Ensure directory exists
if (!fs.existsSync(CONFIG_DIR)) {
    fs.mkdirSync(CONFIG_DIR, { recursive: true });
}

// Migration Logic: Move tasks.json from ~/.mint to ~/.config/mint
if (!fs.existsSync(TASKS_FILE)) {
    const legacyPath = path.join(os.homedir(), '.mint', 'tasks.json');
    if (fs.existsSync(legacyPath)) {
        try {
            fs.copyFileSync(legacyPath, TASKS_FILE);
            console.log('[TaskManager] Migrated tasks from ~/.mint');
        } catch (e) { console.error('[TaskManager] Migration failed:', e); }
    }
}

/**
 * Task Statuses:
 * - 'pending': Waiting for agent to pick up
 * - 'running': Agent is currently working on it
 * - 'completed': Done
 * - 'failed': Error occurred
 */

function readTasks() {
    if (!fs.existsSync(TASKS_FILE)) return [];
    try {
        const content = fs.readFileSync(TASKS_FILE, 'utf8');
        return JSON.parse(content);
    } catch (e) {
        console.error('[TaskManager] Error reading tasks:', e.message);
        return [];
    }
}

function writeTasks(tasks) {
    try {
        fs.writeFileSync(TASKS_FILE, JSON.stringify(tasks, null, 2));
    } catch (e) {
        console.error('[TaskManager] Error writing tasks:', e.message);
    }
}

function addTask(description) {
    const tasks = readTasks();
    const newTask = {
        id: Date.now().toString(),
        description,
        status: 'pending',
        createdAt: new Date().toISOString(),
        updatedAt: new Date().toISOString(),
        steps: [],
        subtasks: [],
        checkpoints: [],
        artifacts: [],
        retryCount: 0,
        maxRetries: 1,
        lastCheckpointAt: null,
        result: null
    };
    tasks.push(newTask);
    writeTasks(tasks);
    return newTask;
}

function getPendingTask() {
    const tasks = readTasks();
    return tasks.find(t => t.status === 'pending');
}

function updateTask(id, updates) {
    const tasks = readTasks();
    const idx = tasks.findIndex(t => t.id === id);
    if (idx !== -1) {
        tasks[idx] = { ...tasks[idx], ...updates, updatedAt: new Date().toISOString() };
        writeTasks(tasks);
        return tasks[idx];
    }
    return null;
}

function getTask(id) {
    return readTasks().find(t => t.id === id) || null;
}

function normalizeTask(task) {
    return {
        ...task,
        steps: Array.isArray(task.steps) ? task.steps : [],
        subtasks: Array.isArray(task.subtasks) ? task.subtasks : [],
        checkpoints: Array.isArray(task.checkpoints) ? task.checkpoints : [],
        artifacts: Array.isArray(task.artifacts) ? task.artifacts : [],
        retryCount: Number.isFinite(task.retryCount) ? task.retryCount : 0,
        maxRetries: Number.isFinite(task.maxRetries) ? task.maxRetries : 1
    };
}

function mutateTask(id, mutator) {
    const tasks = readTasks();
    const idx = tasks.findIndex(t => t.id === id);
    if (idx === -1) return null;
    const next = normalizeTask(tasks[idx]);
    mutator(next);
    next.updatedAt = new Date().toISOString();
    tasks[idx] = next;
    writeTasks(tasks);
    return next;
}

function addSubtask(taskId, title, extra = {}) {
    return mutateTask(taskId, task => {
        task.subtasks.push({
            id: `${taskId}-${task.subtasks.length + 1}`,
            title,
            status: extra.status || 'pending',
            createdAt: new Date().toISOString(),
            updatedAt: new Date().toISOString(),
            ...extra
        });
    });
}

function updateSubtask(taskId, subtaskId, updates = {}) {
    return mutateTask(taskId, task => {
        const subtask = task.subtasks.find(item => item.id === subtaskId);
        if (!subtask) return;
        Object.assign(subtask, updates, { updatedAt: new Date().toISOString() });
    });
}

function addCheckpoint(taskId, checkpoint = {}) {
    return mutateTask(taskId, task => {
        const entry = {
            id: `${taskId}-checkpoint-${task.checkpoints.length + 1}`,
            time: new Date().toISOString(),
            ...checkpoint
        };
        task.checkpoints.push(entry);
        task.lastCheckpointAt = entry.time;
        task.steps.push(entry);
    });
}

function addArtifact(taskId, artifact = {}) {
    return mutateTask(taskId, task => {
        task.artifacts.push({
            id: `${taskId}-artifact-${task.artifacts.length + 1}`,
            time: new Date().toISOString(),
            ...artifact
        });
    });
}

function failTaskWithRetry(id, errorMessage) {
    return mutateTask(id, task => {
        const retryCount = Number(task.retryCount) || 0;
        const maxRetries = Number.isFinite(task.maxRetries) ? task.maxRetries : 1;
        task.result = errorMessage;
        task.retryCount = retryCount + 1;
        task.status = task.retryCount <= maxRetries ? 'pending' : 'failed';
        const checkpoint = {
            id: `${id}-checkpoint-${task.checkpoints.length + 1}`,
            time: new Date().toISOString(),
            phase: task.status === 'pending' ? 'retry_scheduled' : 'failed',
            message: errorMessage,
            retryCount: task.retryCount,
            maxRetries
        };
        task.checkpoints.push(checkpoint);
        task.steps.push(checkpoint);
    });
}

function resumeRunningTasks() {
    const resumed = [];
    const tasks = readTasks().map(task => {
        if (task.status !== 'running') return task;
        const normalized = normalizeTask(task);
        normalized.status = 'pending';
        const checkpoint = {
            id: `${normalized.id}-checkpoint-${normalized.checkpoints.length + 1}`,
            time: new Date().toISOString(),
            phase: 'resume_after_restart',
            message: 'Task was running during shutdown and has been re-queued.'
        };
        normalized.checkpoints.push(checkpoint);
        normalized.steps.push(checkpoint);
        normalized.updatedAt = new Date().toISOString();
        resumed.push(normalized);
        return normalized;
    });
    writeTasks(tasks);
    return resumed;
}

function clearCompletedTasks() {
    const tasks = readTasks();
    const activeTasks = tasks.filter(t => t.status === 'pending' || t.status === 'running');
    writeTasks(activeTasks);
}

module.exports = {
    addTask,
    addArtifact,
    addCheckpoint,
    addSubtask,
    failTaskWithRetry,
    getTask,
    getPendingTask,
    resumeRunningTasks,
    updateTask,
    updateSubtask,
    readTasks,
    clearCompletedTasks
};
