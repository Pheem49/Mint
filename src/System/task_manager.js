const fs = require('fs');
const path = require('path');
const os = require('os');

// Standard location for Mint tasks
const MINT_DIR = path.join(os.homedir(), '.mint');
const TASKS_FILE = path.join(MINT_DIR, 'tasks.json');

// Ensure directory exists
if (!fs.existsSync(MINT_DIR)) {
    fs.mkdirSync(MINT_DIR, { recursive: true });
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

function clearCompletedTasks() {
    const tasks = readTasks();
    const activeTasks = tasks.filter(t => t.status === 'pending' || t.status === 'running');
    writeTasks(activeTasks);
}

module.exports = {
    addTask,
    getPendingTask,
    updateTask,
    readTasks,
    clearCompletedTasks
};
