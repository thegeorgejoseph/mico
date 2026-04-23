const { contextBridge, ipcRenderer } = require("electron");

function resolveBackendOrigin() {
  const backendOriginArgument = process.argv.find((value) => value.startsWith("--mico-backend-origin="));
  if (!backendOriginArgument) {
    throw new Error("mico backend origin was not provided to the preload bridge");
  }
  return backendOriginArgument.slice("--mico-backend-origin=".length);
}

const backendOrigin = resolveBackendOrigin();
const baseURL = `${backendOrigin}/api`;

function terminalSocketURL(sessionId) {
  const url = new URL(`/api/sessions/${sessionId}/terminal/ws`, backendOrigin);
  url.protocol = url.protocol === "https:" ? "wss:" : "ws:";
  return url.toString();
}

async function request(path, options = {}) {
  let response;
  try {
    response = await fetch(`${baseURL}${path}`, {
      headers: { "Content-Type": "application/json" },
      ...options,
    });
  } catch (error) {
    throw new Error(`Backend unavailable for ${path}: ${error.message}`);
  }
  const payload = await response.json().catch(() => ({}));
  if (!response.ok) {
    throw new Error(payload.error || `${options.method || "GET"} ${path} failed with ${response.status}`);
  }
  return payload;
}

contextBridge.exposeInMainWorld("mico", {
  platform: process.platform,
  appInfo: () => ipcRenderer.invoke("mico:app-info"),
  checkForUpdates: () => ipcRenderer.invoke("mico:check-for-updates"),
  openUpdate: (targetURL) => ipcRenderer.invoke("mico:open-update", targetURL),
  state: () => request("/state"),
  doctor: () => request("/doctor"),
  branches: (repoId) => request(`/repos/${repoId}/branches`),
  refreshRepo: (repoId) => request(`/repos/${repoId}/refresh`, { method: "POST" }),
  captureTerminal: (sessionId) => request(`/sessions/${sessionId}/terminal?lines=220`),
  logs: () => request("/logs?limit=80"),
  terminalSocketURL,
  addRepo: (input) => request("/repos", { method: "POST", body: JSON.stringify(input) }),
  runAgent: (input) => request("/agent/run", { method: "POST", body: JSON.stringify(input) }),
  createWorktree: (input) => request("/worktrees", { method: "POST", body: JSON.stringify(input) }),
  pickRepoFolder: () => ipcRenderer.invoke("mico:pick-repo-folder"),
  searchWorkspace: (query, limit = 8) => request(`/navigation/search?q=${encodeURIComponent(query)}&limit=${limit}`),
  focusWorkspace: (input) => request("/navigation/focus", { method: "PUT", body: JSON.stringify(input) }),
  dismissNotification: (id) => request(`/notifications/${id}`, { method: "DELETE" }),
  sendTerminalInput: (sessionId, text) =>
    request(`/sessions/${sessionId}/terminal/input`, { method: "POST", body: JSON.stringify({ text }) }),
  startSession: (input) => request("/sessions", { method: "POST", body: JSON.stringify(input) }),
  stopSession: (sessionId) => request(`/sessions/${sessionId}/stop`, { method: "POST" }),
  resumeSession: (sessionId) => request(`/sessions/${sessionId}/resume`, { method: "POST" }),
});
