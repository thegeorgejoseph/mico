import { useEffect, useEffectEvent } from "react";
import type { CSSProperties, KeyboardEvent as ReactKeyboardEvent, PointerEvent as ReactPointerEvent } from "react";

import micoMarkUrl from "../../assets/mico-mark.svg";
import { AppSidebar } from "../components/AppSidebar";
import { CommandPalette } from "../components/CommandPalette";
import { Modal } from "../components/Modal";
import { NotificationDrawer } from "../components/NotificationDrawer";
import { PreferencesPanel } from "../components/PreferencesPanel";
import { RepoForm, SplashScreen, WorktreeCreator } from "../components/SidebarPanels";
import { WorkspaceView } from "../components/WorkspaceView";
import { INSPECTOR_MAX_WIDTH, INSPECTOR_MIN_WIDTH, MICO_ASCII_WORDMARK, SIDEBAR_COLLAPSED_WIDTH, SIDEBAR_MAX_WIDTH, SIDEBAR_MIN_WIDTH } from "./constants";
import { useLayoutPreferences } from "./hooks/useLayoutPreferences";
import { useSplashScreen } from "./hooks/useSplashScreen";
import { useDesktopApp } from "./useDesktopApp";
import { clamp } from "./utils";
import "./styles/app.scss";

export function App() {
  const desktop = useDesktopApp();
  const layout = useLayoutPreferences(desktop.micoState.repos.length);
  const splash = useSplashScreen();

  const handleGlobalKeyDown = useEffectEvent((event: KeyboardEvent) => {
    if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "k") {
      event.preventDefault();
      desktop.openCommandPalette();
      desktop.setPreferences((current) => ({ ...current, open: false }));
    }
    if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "b") {
      event.preventDefault();
      layout.setSidebarCollapsed((current) => !current);
    }
    if ((event.metaKey || event.ctrlKey) && event.key === ",") {
      event.preventDefault();
      if (desktop.overlays.commandPaletteOpen && desktop.agentRunPending) {
        return;
      }
      desktop.setPreferences({ open: true, tab: "app" });
      desktop.dismissCommandPalette();
      desktop.setOverlays((current) => ({ ...current, notificationsOpen: false }));
    }
    if (event.key === "Escape") {
      desktop.dismissCommandPalette();
      desktop.setOverlays((current) => ({ ...current, notificationsOpen: false }));
      desktop.setPreferences((current) => ({ ...current, open: false }));
    }
  });

  useEffect(() => {
    function handleKeyDown(event: KeyboardEvent) {
      handleGlobalKeyDown(event);
    }

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [handleGlobalKeyDown]);

  function beginHorizontalResize(kind: "sidebar" | "inspector", event: ReactPointerEvent<HTMLDivElement>) {
    if (kind === "sidebar" && layout.sidebarCollapsed) {
      return;
    }

    event.preventDefault();
    const startX = event.clientX;
    const startWidth = kind === "sidebar" ? layout.sidebarWidth : layout.inspectorWidth;
    const minWidth = kind === "sidebar" ? SIDEBAR_MIN_WIDTH : INSPECTOR_MIN_WIDTH;
    const maxWidth = kind === "sidebar" ? SIDEBAR_MAX_WIDTH : INSPECTOR_MAX_WIDTH;
    const updateWidth = kind === "sidebar" ? layout.setSidebarWidth : layout.setInspectorWidth;

    document.body.classList.add("is-resizing");
    event.currentTarget.setPointerCapture?.(event.pointerId);

    function handlePointerMove(moveEvent: PointerEvent) {
      updateWidth(clamp(startWidth + (moveEvent.clientX - startX), minWidth, maxWidth));
    }

    function handlePointerUp() {
      document.body.classList.remove("is-resizing");
      window.removeEventListener("pointermove", handlePointerMove);
      window.removeEventListener("pointerup", handlePointerUp);
    }

    window.addEventListener("pointermove", handlePointerMove);
    window.addEventListener("pointerup", handlePointerUp, { once: true });
  }

  function handleProjectSearchKeyDown(event: ReactKeyboardEvent<HTMLInputElement>) {
    if (!desktop.search.results.length) {
      return;
    }
    if (event.key === "ArrowDown" || event.key === "j") {
      event.preventDefault();
      desktop.search.setHighlightedIndex((current) => (current + 1) % desktop.search.results.length);
    }
    if (event.key === "ArrowUp" || event.key === "k") {
      event.preventDefault();
      desktop.search.setHighlightedIndex((current) => (current - 1 + desktop.search.results.length) % desktop.search.results.length);
    }
    if (event.key === "Enter") {
      event.preventDefault();
      desktop.launchSafely(
        () => desktop.actions.selectSidebarResult(desktop.search.results[desktop.search.highlightedIndex] ?? desktop.search.results[0]),
        "Unable to select search result",
      );
    }
    if (event.key === "Escape") {
      desktop.setSidebarSearch("");
      desktop.search.setHighlightedIndex(0);
    }
  }

  const appShellStyle = { "--sidebar-width": `${layout.sidebarCollapsed ? SIDEBAR_COLLAPSED_WIDTH : layout.sidebarWidth}px` } as CSSProperties;
  const contentGridStyle = { "--inspector-width": `${layout.inspectorWidth}px` } as CSSProperties;

  return (
    <>
      <div className={`app-shell ${desktop.overlays.commandPaletteOpen || desktop.overlays.notificationsOpen || desktop.preferences.open ? "has-overlay" : ""} ${layout.sidebarCollapsed ? "is-sidebar-collapsed" : ""}`} style={appShellStyle}>
        <AppSidebar
          appVersion={desktop.appInfo ? `v${desktop.appInfo.version}` : "v1.0.0"}
          micoMarkUrl={micoMarkUrl}
          onAddProject={() => desktop.setOverlays((current) => ({ ...current, modal: "repo" }))}
          onOpenSettings={() => desktop.setPreferences({ open: true, tab: "app" })}
          onProjectSearchChange={(value) => {
            desktop.setSidebarSearch(value);
            desktop.search.setHighlightedIndex(0);
          }}
          onProjectSearchKeyDown={handleProjectSearchKeyDown}
          onSelectProject={(id) => {
            desktop.launchSafely(() => desktop.actions.selectRepo(id), "Unable to select project");
          }}
          onSelectSearchResult={(result) => {
            desktop.launchSafely(() => desktop.actions.selectSidebarResult(result), "Unable to select search result");
          }}
          onToggleCollapse={() => layout.setSidebarCollapsed((current) => !current)}
          onToggleProjectsExpanded={() => layout.setRepoNavExpanded((current) => !current)}
          projects={desktop.micoState.repos}
          projectsExpanded={layout.repoNavExpanded}
          searchInputRef={desktop.sidebarSearchRef}
          searchQuery={desktop.sidebarSearch}
          searchResults={desktop.search.results}
          selectedProjectId={desktop.focus.repoId}
          selectedSearchIndex={desktop.search.highlightedIndex}
          sidebarCollapsed={layout.sidebarCollapsed}
          status={desktop.status}
        />
        {!layout.sidebarCollapsed ? (
          <div
            aria-label="Resize sidebar"
            className="resize-handle resize-handle--sidebar"
            onPointerDown={(event) => beginHorizontalResize("sidebar", event)}
            role="separator"
          />
        ) : null}

        <WorkspaceView
          activityOpen={desktop.overlays.activityOpen}
          agent={desktop.sessionAgent}
          branchesCount={desktop.branches.length}
          canResumeSession={desktop.selectedSession?.status === "exited" || desktop.selectedSession?.status === "failed"}
          canStopSession={desktop.selectedSession?.status === "running"}
          contentGridStyle={contentGridStyle}
          logs={desktop.micoState.logs}
          notificationsCount={desktop.unreadNotifications}
          onAddWorktree={() => desktop.setOverlays((current) => ({ ...current, modal: "worktree" }))}
          onOpenMission={desktop.openCommandPalette}
          onRefreshProject={() => desktop.launchSafely(desktop.actions.refreshSelectedRepo, "Unable to refresh project")}
          onResumeSession={desktop.resumeSession}
          onSelectSession={(id) => {
            desktop.launchSafely(() => desktop.actions.selectSession(id), "Unable to select session");
          }}
          onSelectWorktree={(id) => {
            desktop.launchSafely(() => desktop.actions.selectWorktree(id), "Unable to select worktree");
          }}
          onStartSession={desktop.startSession}
          onStopSession={desktop.stopSession}
          onToggleActivity={() => desktop.setOverlays((current) => ({ ...current, activityOpen: !current.activityOpen }))}
          onToggleInspectorResize={(event) => beginHorizontalResize("inspector", event)}
          onToggleNotifications={() => desktop.setOverlays((current) => ({ ...current, notificationsOpen: !current.notificationsOpen }))}
          repoWorktrees={desktop.repoWorktrees}
          selectedRepo={desktop.selectedRepo}
          selectedSession={desktop.selectedSession}
          selectedWorktree={desktop.selectedWorktree}
          sessionsForWorktree={desktop.sessionsForWorktree}
          setAgent={desktop.setSessionAgent}
          statusNotice={desktop.statusNotice}
          theme={layout.theme}
        />

        <NotificationDrawer
          notifications={desktop.micoState.notifications}
          onClose={() => desktop.setOverlays((current) => ({ ...current, notificationsOpen: false }))}
          onSeen={desktop.dismissNotification}
          open={desktop.overlays.notificationsOpen}
        />
      </div>

      <CommandPalette
        errorMessage={desktop.agentRunError}
        input={desktop.commandInput}
        isRunning={desktop.agentRunPending}
        onClose={desktop.dismissCommandPalette}
        onInputChange={desktop.setCommandInput}
        onProviderChange={desktop.setCommandProvider}
        onRun={desktop.actions.runAgentCommand}
        open={desktop.overlays.commandPaletteOpen}
        provider={desktop.commandProvider}
      />

      <PreferencesPanel
        appInfo={desktop.appInfo}
        doctorLoading={desktop.doctorLoading}
        doctorReport={desktop.doctorReport}
        onCheckForUpdates={() => desktop.launchSafely(() => desktop.actions.checkForUpdates(), "Unable to check for updates")}
        onClose={() => desktop.setPreferences((current) => ({ ...current, open: false }))}
        onOpenUpdate={() => desktop.launchSafely(desktop.openUpdateFlow, "Unable to open update")}
        onRefreshDoctor={() => desktop.launchSafely(desktop.loadDoctorReport, "Unable to refresh doctor report")}
        onSelectTab={(tab) => desktop.setPreferences((current) => ({ ...current, tab }))}
        onThemeChange={layout.setTheme}
        open={desktop.preferences.open}
        selectedTab={desktop.preferences.tab}
        theme={layout.theme}
        updateInfo={desktop.updateInfo}
        updateLoading={desktop.updateLoading}
      />

      <Modal open={desktop.overlays.modal === "repo"} subtitle="Track an existing local git project." title="Add Project" onClose={() => desktop.setOverlays((current) => ({ ...current, modal: null }))}>
        <RepoForm
          onPickFolder={desktop.actions.pickRepoFolder}
          onSubmit={(event) => {
            event.preventDefault();
            desktop.launchSafely(desktop.actions.addRepo, "Unable to add project");
          }}
          repoName={desktop.repoDraft.name}
          repoPath={desktop.repoDraft.path}
          setRepoName={(name) => desktop.setRepoDraft((current) => ({ ...current, name }))}
          setRepoPath={(path) => desktop.setRepoDraft((current) => ({ ...current, path }))}
        />
      </Modal>
      <Modal open={desktop.overlays.modal === "worktree"} subtitle={desktop.selectedRepo ? `Create a new branch-backed worktree for ${desktop.selectedRepo.name}.` : "Select a project first."} title="Create Worktree" onClose={() => desktop.setOverlays((current) => ({ ...current, modal: null }))}>
        <WorktreeCreator
          base={desktop.worktreeDraft.base}
          branch={desktop.worktreeDraft.branch}
          branches={desktop.branches}
          branchesLoading={desktop.branchesLoading}
          disabled={!desktop.focus.repoId}
          existing={desktop.worktreeDraft.existing}
          onBaseChange={(base) => desktop.setWorktreeDraft((current) => ({ ...current, base }))}
          onBranchChange={(branch) => desktop.setWorktreeDraft((current) => ({ ...current, branch }))}
          onExistingChange={(existing) => desktop.setWorktreeDraft((current) => ({ ...current, existing }))}
          onSubmit={(event) => {
            event.preventDefault();
            desktop.launchSafely(desktop.actions.createWorktree, "Unable to create worktree");
          }}
        />
      </Modal>

      {splash.visible ? <SplashScreen appInfo={desktop.appInfo} error={desktop.error} fading={splash.fading} markUrl={micoMarkUrl} status={desktop.status} wordmark={MICO_ASCII_WORDMARK} /> : null}
    </>
  );
}
