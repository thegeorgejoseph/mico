import { useEffect, useMemo, useRef, useState } from "react";

import { DEFAULT_AGENT, DEFAULT_COMMAND_PROVIDER, emptyState } from "./constants";
import { useWorkspaceSearch } from "./hooks/useWorkspaceSearch";
import { getErrorMessage } from "./utils";
import { normalizeLogEvents, normalizeMicoState, normalizeStringList } from "../lib/state";
import type { StatusNotice } from "../components/StatusChip";
import { compareSessions } from "../lib/sessionSelection";
import { deriveViewSelection } from "../lib/viewSelection";
import type { AgentKind, AppInfo, DoctorReport, MicoState, UpdateInfo, WorkspaceFocus, WorkspaceSearchResult } from "../types";

interface RepoDraft {
  name: string;
  path: string;
}

interface WorktreeDraft {
  base: string;
  branch: string;
  existing: boolean;
}

interface PreferencesState {
  open: boolean;
  tab: "app" | "theme" | "doctor";
}

interface OverlayState {
  activityOpen: boolean;
  commandPaletteOpen: boolean;
  modal: "repo" | "worktree" | null;
  notificationsOpen: boolean;
}

export function useDesktopApp() {
  const [micoState, setMicoState] = useState<MicoState>(emptyState);
  const [focus, setFocus] = useState<WorkspaceFocus>(emptyState.selection);
  const [repoDraft, setRepoDraft] = useState<RepoDraft>({ name: "", path: "" });
  const [worktreeDraft, setWorktreeDraft] = useState<WorktreeDraft>({ base: "", branch: "", existing: false });
  const [branches, setBranches] = useState<string[]>([]);
  const [branchesLoading, setBranchesLoading] = useState(false);
  const [sessionAgent, setSessionAgent] = useState<AgentKind>(DEFAULT_AGENT);
  const [commandProvider, setCommandProviderState] = useState<AgentKind>(DEFAULT_COMMAND_PROVIDER);
  const [commandInput, setCommandInputState] = useState("");
  const [agentRunError, setAgentRunError] = useState("");
  const [agentRunPending, setAgentRunPending] = useState(false);
  const [preferences, setPreferences] = useState<PreferencesState>({ open: false, tab: "theme" });
  const [overlays, setOverlays] = useState<OverlayState>({
    activityOpen: false,
    commandPaletteOpen: false,
    modal: null,
    notificationsOpen: false,
  });
  const [statusNotice, setStatusNotice] = useState<StatusNotice | null>(null);
  const [status, setStatus] = useState("Getting things ready...");
  const [error, setError] = useState("");
  const [sidebarSearch, setSidebarSearch] = useState("");
  const [doctorReport, setDoctorReport] = useState<DoctorReport | null>(null);
  const [doctorLoading, setDoctorLoading] = useState(false);
  const [appInfo, setAppInfo] = useState<AppInfo | null>(null);
  const [updateInfo, setUpdateInfo] = useState<UpdateInfo | null>(null);
  const [updateLoading, setUpdateLoading] = useState(false);
  const sidebarSearchRef = useRef<HTMLInputElement>(null);
  const search = useWorkspaceSearch(sidebarSearch);

  const selectedRepoId = focus.repoId;
  const selectedWorktreeId = focus.worktreeId;
  const selectedSessionId = focus.sessionId;

  const selectedRepo = useMemo(
    () => micoState.repos.find((repo) => repo.id === selectedRepoId) ?? null,
    [micoState.repos, selectedRepoId],
  );
  const selectedWorktree = useMemo(
    () => micoState.worktrees.find((worktree) => worktree.id === selectedWorktreeId) ?? null,
    [micoState.worktrees, selectedWorktreeId],
  );
  const repoWorktrees = useMemo(
    () => micoState.worktrees.filter((worktree) => worktree.repoId === selectedRepoId),
    [micoState.worktrees, selectedRepoId],
  );
  const sessionsForWorktree = useMemo(
    () => micoState.sessions.filter((session) => session.worktreeId === selectedWorktreeId).sort(compareSessions),
    [micoState.sessions, selectedWorktreeId],
  );
  const selectedSession = useMemo(
    () => micoState.sessions.find((session) => session.id === selectedSessionId) ?? sessionsForWorktree[0] ?? null,
    [micoState.sessions, selectedSessionId, sessionsForWorktree],
  );
  const unreadNotifications = useMemo(
    () => micoState.notifications.filter((notification) => !notification.seen).length,
    [micoState.notifications],
  );

  function applyFocus(next: WorkspaceFocus) {
    setFocus(next);
  }

  function reportRuntimeError(caught: unknown, fallback: string) {
    const message = getErrorMessage(caught, fallback);
    console.error(message, caught);
    setError(message);
    setStatusNotice({ tone: "error", message });
  }

  function launchSafely(task: () => Promise<void>, fallback: string) {
    void task().catch((caught) => {
      reportRuntimeError(caught, fallback);
    });
  }

  async function loadState() {
    const next = normalizeMicoState(await window.mico.state());
    const nextSelection = deriveViewSelection(next);
    setMicoState(next);
    setFocus({ ...nextSelection, mode: next.selection.mode || "effort" });
    setStatus("Ready");
  }

  async function refreshLogs() {
    const nextLogs = normalizeLogEvents(await window.mico.logs());
    setMicoState((current) => ({ ...current, logs: nextLogs }));
  }

  async function loadAppInfo() {
    const next = await window.mico.appInfo();
    setAppInfo(next);
  }

  async function checkForUpdates(options?: { quiet?: boolean }) {
    setUpdateLoading(true);
    if (!options?.quiet) {
      setError("");
      setStatusNotice({ tone: "working", message: "Checking for updates" });
    }
    try {
      const next = await window.mico.checkForUpdates();
      setUpdateInfo(next);
      if (!options?.quiet) {
        setStatusNotice({
          tone: "ok",
          message: next.status === "unpublished" ? "No published desktop release yet" : next.available ? `mico ${next.latestVersion} is ready` : "mico is up to date",
        });
      }
    } catch (caught) {
      if (!options?.quiet) {
        const message = getErrorMessage(caught, "Unable to check for updates");
        setError(message);
        setStatusNotice({ tone: "error", message });
      }
    } finally {
      setUpdateLoading(false);
    }
  }

  async function runAction(label: string, action: () => Promise<void>, options?: { quiet?: boolean; successLabel?: string }) {
    setError("");
    if (!options?.quiet) {
      setStatusNotice({ tone: "working", message: label });
    }
    try {
      await action();
      if (!options?.quiet) {
        setStatusNotice({ tone: "ok", message: options?.successLabel ?? label });
      }
    } catch (caught) {
      const message = getErrorMessage(caught, "Something went wrong");
      setError(message);
      setStatusNotice({ tone: "error", message });
    } finally {
      launchSafely(refreshLogs, "Unable to refresh activity");
    }
  }

  useEffect(() => {
    let cancelled = false;

    async function loadWithRetry() {
      for (let attempt = 1; attempt <= 20; attempt += 1) {
        try {
          if (!cancelled) {
            await loadState();
          }
          return;
        } catch (caught) {
          if (cancelled) {
            return;
          }
          setStatus(attempt === 1 ? "Opening your workspace..." : `Still getting things ready (${attempt}/20)`);
          await new Promise((resolve) => setTimeout(resolve, 350));
          if (attempt === 20) {
            setError(getErrorMessage(caught, "Backend unavailable"));
          }
        }
      }
    }

    launchSafely(loadWithRetry, "Unable to load application state");
    launchSafely(loadAppInfo, "Unable to load app metadata");
    launchSafely(() => checkForUpdates({ quiet: true }), "Unable to check for updates");
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    function handleWindowError(event: ErrorEvent) {
      reportRuntimeError(event.error ?? new Error(event.message), "An unexpected renderer error occurred");
    }

    function handleUnhandledRejection(event: PromiseRejectionEvent) {
      reportRuntimeError(event.reason, "An operation failed unexpectedly");
    }

    window.addEventListener("error", handleWindowError);
    window.addEventListener("unhandledrejection", handleUnhandledRejection);
    return () => {
      window.removeEventListener("error", handleWindowError);
      window.removeEventListener("unhandledrejection", handleUnhandledRejection);
    };
  }, []);

  useEffect(() => {
    document.body.dataset.platform = window.mico.platform;
    return () => {
      delete document.body.dataset.platform;
    };
  }, []);

  useEffect(() => {
    if (!statusNotice || statusNotice.tone !== "ok") {
      return undefined;
    }
    const timer = window.setTimeout(() => setStatusNotice((current) => (current?.tone === "ok" ? null : current)), 2200);
    return () => window.clearTimeout(timer);
  }, [statusNotice]);

  useEffect(() => {
    let cancelled = false;

    async function tick() {
      try {
        const nextLogs = normalizeLogEvents(await window.mico.logs());
        if (!cancelled) {
          setMicoState((current) => ({ ...current, logs: nextLogs }));
        }
      } catch {
        // Startup retry path owns availability messaging.
      }
    }

    void tick();
    const timer = window.setInterval(tick, 2500);
    return () => {
      cancelled = true;
      window.clearInterval(timer);
    };
  }, []);

  useEffect(() => {
    if (!selectedRepoId) {
      setBranches([]);
      setWorktreeDraft((current) => ({ ...current, base: "" }));
      setBranchesLoading(false);
      return;
    }

    let cancelled = false;
    setBranchesLoading(true);
    setError("");
    void loadBranchesForRepo(selectedRepoId)
      .then((nextBranches) => {
        if (!cancelled) {
          setBranches(nextBranches);
          setWorktreeDraft((current) => ({ ...current, base: current.base || nextBranches[0] || "main" }));
        }
      })
      .catch((caught) => {
        if (!cancelled) {
          const message = getErrorMessage(caught, "Unable to load branches");
          setError(message);
          setStatusNotice({ tone: "error", message });
        }
      })
      .finally(() => {
        if (!cancelled) {
          setBranchesLoading(false);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [selectedRepoId]);

  useEffect(() => {
    if (!preferences.open || preferences.tab !== "doctor") {
      return;
    }
    launchSafely(loadDoctorReport, "Unable to refresh doctor report");
  }, [preferences.open, preferences.tab]);

  async function loadBranchesForRepo(repoId: string) {
    return normalizeStringList(await window.mico.branches(repoId));
  }

  async function focusWorkspace(input: Partial<WorkspaceFocus>) {
    const nextSelection = await window.mico.focusWorkspace({ ...input, mode: input.mode ?? focus.mode });
    applyFocus(nextSelection);
    return nextSelection;
  }

  async function pickRepoFolder() {
    try {
      const path = await window.mico.pickRepoFolder();
      if (path) {
        setRepoDraft({
          name: path.split("/").filter(Boolean).at(-1) ?? "",
          path,
        });
      }
    } catch (caught) {
      reportRuntimeError(caught, "Unable to choose a project folder");
    }
  }

  async function addRepo() {
    await runAction("Adding project", async () => {
      const repo = await window.mico.addRepo({ path: repoDraft.path, name: repoDraft.name || undefined });
      setRepoDraft({ name: "", path: "" });
      setOverlays((current) => ({ ...current, modal: null }));
      await focusWorkspace({ repoId: repo.id, worktreeId: "", sessionId: "" });
      await loadState();
    });
  }

  async function createWorktree() {
    await runAction(worktreeDraft.existing ? "Creating worktree from existing branch" : "Creating worktree", async () => {
      const worktree = await window.mico.createWorktree({
        repoId: selectedRepoId,
        branch: worktreeDraft.branch,
        base: worktreeDraft.base,
        existing: worktreeDraft.existing,
      });
      setWorktreeDraft((current) => ({ ...current, branch: "", existing: false }));
      setOverlays((current) => ({ ...current, modal: null }));
      await selectWorktree(worktree.id);
      await loadState();
    });
  }

  async function startSession() {
    if (!selectedWorktree) {
      return;
    }
    await runAction(`Starting ${sessionAgent}`, async () => {
      const session = await window.mico.startSession({ worktreeId: selectedWorktree.id, agent: sessionAgent });
      await selectSession(session.id);
      await loadState();
    });
  }

  async function stopSession() {
    if (!selectedSession) {
      return;
    }
    await runAction(
      `Stopping ${selectedSession.agent}`,
      async () => {
        await window.mico.stopSession(selectedSession.id);
        await loadState();
      },
      { successLabel: "Session stopped" },
    );
  }

  async function resumeSession() {
    if (!selectedSession) {
      return;
    }
    await runAction(
      `Resuming ${selectedSession.agent}`,
      async () => {
        await window.mico.resumeSession(selectedSession.id);
        await loadState();
      },
      { successLabel: "Session resumed" },
    );
  }

  async function refreshSelectedRepo() {
    if (!selectedRepoId) {
      await loadState();
      return;
    }
    await runAction(
      "Refreshing project",
      async () => {
        await window.mico.refreshRepo(selectedRepoId);
        const nextBranches = await loadBranchesForRepo(selectedRepoId);
        setBranches(nextBranches);
        setWorktreeDraft((current) => ({ ...current, base: current.base || nextBranches[0] || "main" }));
        await loadState();
      },
      { successLabel: "Project refreshed" },
    );
  }

  async function selectRepo(repoId: string) {
    await focusWorkspace({ repoId, worktreeId: "", sessionId: "" });
  }

  async function selectWorktree(worktreeId: string) {
    const worktree = micoState.worktrees.find((candidate) => candidate.id === worktreeId);
    const sessionId = micoState.sessions.filter((session) => session.worktreeId === worktreeId).sort(compareSessions)[0]?.id ?? "";
    await focusWorkspace({ repoId: worktree?.repoId ?? selectedRepoId, worktreeId, sessionId });
  }

  async function selectSession(sessionId: string) {
    await focusWorkspace({ sessionId });
  }

  async function dismissNotification(id: string) {
    await runAction("Dismissing notification", async () => {
      await window.mico.dismissNotification(id);
      await loadState();
    });
  }

  function closeCommandPalette() {
    setAgentRunError("");
    setOverlays((current) => ({ ...current, commandPaletteOpen: false }));
  }

  function dismissCommandPalette() {
    if (agentRunPending) {
      return;
    }
    closeCommandPalette();
  }

  function openCommandPalette() {
    setAgentRunError("");
    setOverlays((current) => ({ ...current, commandPaletteOpen: true, notificationsOpen: false }));
  }

  function setCommandInput(value: string) {
    if (agentRunError) {
      setAgentRunError("");
    }
    setCommandInputState(value);
  }

  function setCommandProvider(value: AgentKind) {
    if (agentRunError) {
      setAgentRunError("");
    }
    setCommandProviderState(value);
  }

  async function runAgentCommand() {
    if (!commandInput.trim() || agentRunPending) {
      return;
    }

    setAgentRunPending(true);
    setAgentRunError("");
    setError("");
    setStatusNotice({ tone: "working", message: `Running ${commandProvider}` });

    try {
      const response = await window.mico.runAgent({ provider: commandProvider, message: commandInput });
      setCommandInputState("");
      setAgentRunError("");
      closeCommandPalette();
      if (response.selection) {
        applyFocus(response.selection);
      }
      await loadState();
      setStatusNotice({ tone: "ok", message: `${commandProvider} action applied` });
    } catch (caught) {
      const message = getErrorMessage(caught, "Unable to run agent command");
      setError(message);
      setAgentRunError(message);
      setStatusNotice({ tone: "error", message });
    } finally {
      setAgentRunPending(false);
      launchSafely(refreshLogs, "Unable to refresh activity");
    }
  }

  async function selectSidebarResult(result: WorkspaceSearchResult) {
    setSidebarSearch("");
    search.setHighlightedIndex(0);
    if (result.kind === "repo") {
      await selectRepo(result.id);
      return;
    }
    await selectWorktree(result.id);
  }

  async function loadDoctorReport() {
    setDoctorLoading(true);
    try {
      const report = await window.mico.doctor();
      setDoctorReport(report);
    } catch (caught) {
      const message = getErrorMessage(caught, "Unable to load doctor report");
      setError(message);
      setStatusNotice({ tone: "error", message });
    } finally {
      setDoctorLoading(false);
    }
  }

  async function openUpdateFlow() {
    const targetURL = updateInfo?.downloadURL || updateInfo?.releaseURL || appInfo?.releaseURL;
    await runAction(
      "Opening update download",
      async () => {
        await window.mico.openUpdate(targetURL);
      },
      { successLabel: "Update download opened" },
    );
  }

  return {
    appInfo,
    branches,
    branchesLoading,
    agentRunError,
    agentRunPending,
    commandInput,
    commandProvider,
    closeCommandPalette,
    dismissCommandPalette,
    dismissNotification,
    doctorLoading,
    doctorReport,
    error,
    focus,
    launchSafely,
    loadDoctorReport,
    micoState,
    openUpdateFlow,
    openCommandPalette,
    overlays,
    preferences,
    repoDraft,
    repoWorktrees,
    search,
    selectedRepo,
    selectedSession,
    selectedWorktree,
    sessionAgent,
    sessionsForWorktree,
    setCommandInput,
    setCommandProvider,
    setOverlays,
    setPreferences,
    setRepoDraft,
    setSessionAgent,
    setSidebarSearch,
    setWorktreeDraft,
    sidebarSearch,
    sidebarSearchRef,
    resumeSession,
    startSession,
    status,
    statusNotice,
    stopSession,
    unreadNotifications,
    updateInfo,
    updateLoading,
    worktreeDraft,
    actions: {
      addRepo,
      checkForUpdates,
      createWorktree,
      loadState,
      pickRepoFolder,
      refreshSelectedRepo,
      resumeSession,
      runAgentCommand,
      selectRepo,
      selectSession,
      selectSidebarResult,
      selectWorktree,
    },
  };
}
