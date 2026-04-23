import { PanelLeftClose, Search, Settings2 } from "lucide-react";
import type { KeyboardEvent, RefObject } from "react";

import { TextField } from "./Field";
import { NavSection } from "./SidebarPanels";
import type { Repo, WorkspaceSearchResult } from "../types";

interface AppSidebarProps {
  appVersion: string;
  micoMarkUrl: string;
  onAddProject: () => void;
  onOpenSettings: () => void;
  onProjectSearchChange: (value: string) => void;
  onProjectSearchKeyDown: (event: KeyboardEvent<HTMLInputElement>) => void;
  onSelectProject: (repoId: string) => void;
  onSelectSearchResult: (result: WorkspaceSearchResult) => void;
  onToggleCollapse: () => void;
  onToggleProjectsExpanded: () => void;
  projects: Repo[];
  projectsExpanded: boolean;
  searchInputRef: RefObject<HTMLInputElement | null>;
  searchQuery: string;
  searchResults: WorkspaceSearchResult[];
  selectedProjectId: string;
  selectedSearchIndex: number;
  sidebarCollapsed: boolean;
  status: string;
}

export function AppSidebar({
  appVersion,
  micoMarkUrl,
  onAddProject,
  onOpenSettings,
  onProjectSearchChange,
  onProjectSearchKeyDown,
  onSelectProject,
  onSelectSearchResult,
  onToggleCollapse,
  onToggleProjectsExpanded,
  projects,
  projectsExpanded,
  searchInputRef,
  searchQuery,
  searchResults,
  selectedProjectId,
  selectedSearchIndex,
  sidebarCollapsed,
  status,
}: AppSidebarProps) {
  return (
    <aside className={`sidebar ${sidebarCollapsed ? "is-collapsed" : ""}`}>
      <div className="sidebar__top">
        {sidebarCollapsed ? (
          <button
            className="sidebar-logo-button"
            onClick={() => {
              onToggleCollapse();
              window.setTimeout(() => searchInputRef.current?.focus(), 30);
            }}
            type="button"
            title="Search projects and worktrees"
          >
            <img className="sidebar-logo-button__mark" src={micoMarkUrl} alt="" />
          </button>
        ) : (
          <>
            <div className="brand">
              <div className="brand__identity">
                <img className="brand__mark" src={micoMarkUrl} alt="" />
                <div className="brand__copy">
                  <h1 className="brand__title">mico</h1>
                  <p>Mission control for local agent work.</p>
                </div>
              </div>
              <div className="brand__status">
                <p>{status}</p>
                <button
                  className="brand__collapse"
                  onClick={onToggleCollapse}
                  type="button"
                  aria-label="Collapse sidebar"
                  title="Collapse sidebar (⌘B)"
                >
                  <PanelLeftClose size={13} />
                </button>
              </div>
            </div>

            <div className="sidebar-search">
              <label className="sidebar-search__field">
                <Search size={14} />
                <span className="sidebar-search__label">Search</span>
                <TextField
                  onChange={(event) => onProjectSearchChange(event.target.value)}
                  onKeyDown={onProjectSearchKeyDown}
                  placeholder="Search projects and worktrees"
                  ref={searchInputRef}
                  value={searchQuery}
                />
              </label>
              {searchResults.length ? (
                <div className="sidebar-search__results">
                  {searchResults.map((result, index) => (
                    <button
                      className={`sidebar-search__result ${index === selectedSearchIndex ? "is-highlighted" : ""}`}
                      key={`${result.kind}-${result.id}`}
                      onClick={() => onSelectSearchResult(result)}
                      type="button"
                    >
                      <span className="sidebar-search__result-icon">{result.kind === "repo" ? "P" : "W"}</span>
                      <span className="sidebar-search__result-copy">
                        <strong>{result.label}</strong>
                        <em>{result.meta}</em>
                      </span>
                    </button>
                  ))}
                </div>
              ) : searchQuery.trim() ? (
                <div className="sidebar-search__empty">No matching projects or worktrees.</div>
              ) : null}
            </div>
          </>
        )}
      </div>

      <NavSection
        compact={sidebarCollapsed}
        expanded={projectsExpanded}
        onAdd={onAddProject}
        onSelect={onSelectProject}
        onToggleExpanded={onToggleProjectsExpanded}
        repos={projects}
        selectedRepoId={selectedProjectId}
      />

      <div className="sidebar__footer">
        <button
          className={`sidebar__preferences ${sidebarCollapsed ? "is-compact" : ""}`}
          onClick={onOpenSettings}
          type="button"
          title="Open settings (⌘,)"
        >
          <Settings2 size={15} />
          {!sidebarCollapsed ? (
            <>
              <span>Settings</span>
              <strong>{appVersion}</strong>
            </>
          ) : null}
        </button>
      </div>
    </aside>
  );
}
