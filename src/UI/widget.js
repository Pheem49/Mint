const widgetContainer = document.getElementById('widget-container');
const statusBadge = document.getElementById('status-badge');

if (window.widgetAPI) {
    window.widgetAPI.onStateChange((state) => {
        // state can be: 'idle', 'listening', 'thinking', 'speaking'
        widgetContainer.className = `state-${state}`;
        statusBadge.textContent = state.charAt(0).toUpperCase() + state.slice(1);
    });
}
