import { Gauge, LoaderCircle, RefreshCw, SunMoon, X } from "lucide-react";

import { Button } from "./Button";
import { formatBytes, formatTimestamp } from "../lib/format";
import { handleScopedNavigation } from "../lib/navigation";
import { THEMES, type AppInfo, type DoctorCheck, type DoctorReport, type ThemeName, type UpdateInfo } from "../types";

interface PreferencesPanelProps {
  appInfo: AppInfo | null;
  doctorLoading: boolean;
  doctorReport: DoctorReport | null;
  onCheckForUpdates: () => void;
  onClose: () => void;
  onOpenUpdate: () => void;
  onRefreshDoctor: () => void;
  onSelectTab: (tab: "app" | "theme" | "doctor") => void;
  onThemeChange: (theme: ThemeName) => void;
  open: boolean;
  selectedTab: "app" | "theme" | "doctor";
  theme: ThemeName;
  updateInfo: UpdateInfo | null;
  updateLoading: boolean;
}

export function PreferencesPanel({
  appInfo,
  doctorLoading,
  doctorReport,
  onCheckForUpdates,
  onClose,
  onOpenUpdate,
  onRefreshDoctor,
  onSelectTab,
  onThemeChange,
  open,
  selectedTab,
  theme,
  updateInfo,
  updateLoading,
}: PreferencesPanelProps) {
  if (!open) {
    return null;
  }

  return (
    <div className="modal-backdrop" role="presentation" onMouseDown={onClose}>
      <section className="preferences-panel" role="dialog" aria-modal="true" aria-label="Settings" onMouseDown={(event) => event.stopPropagation()}>
        <div className="preferences-panel__sidebar">
          <div className="preferences-panel__header">
            <h2>Settings</h2>
            <p>Desktop controls and system health.</p>
          </div>
          <div className="preferences-tabs" data-nav-scope="preferences-tabs">
            <button className={selectedTab === "app" ? "is-active" : ""} data-nav-item="true" onClick={() => onSelectTab("app")} onKeyDown={(event) => handleScopedNavigation(event)} type="button">
              <RefreshCw size={14} />
              <span>App</span>
            </button>
            <button className={selectedTab === "theme" ? "is-active" : ""} data-nav-item="true" onClick={() => onSelectTab("theme")} onKeyDown={(event) => handleScopedNavigation(event)} type="button">
              <SunMoon size={14} />
              <span>Theme</span>
            </button>
            <button className={selectedTab === "doctor" ? "is-active" : ""} data-nav-item="true" onClick={() => onSelectTab("doctor")} onKeyDown={(event) => handleScopedNavigation(event)} type="button">
              <Gauge size={14} />
              <span>Doctor</span>
            </button>
          </div>
        </div>
        <div className="preferences-panel__content">
          <div className="preferences-panel__toolbar">
            <div>
              <h2>{selectedTab === "app" ? "App" : selectedTab === "theme" ? "Theme" : "Doctor"}</h2>
              <p>
                {selectedTab === "app"
                  ? "Version details and release updates."
                  : selectedTab === "theme"
                    ? "Choose how mico looks."
                    : "Check the system pieces mico depends on."}
              </p>
            </div>
            <button className="toolbar-icon" onClick={onClose} type="button" aria-label="Close settings" title="Close settings">
              <X size={16} />
            </button>
          </div>
          {selectedTab === "app" ? (
            <div className="preferences-panel__body">
              <div className="settings-section">
                <div className="settings-section__header">
                  <RefreshCw size={15} />
                  <div>
                    <strong>mico {appInfo?.version ?? "1.0.0"}</strong>
                    <span>{appInfo?.packaged ? "Packaged desktop build" : "Development desktop build"}</span>
                  </div>
                </div>
                <div className="app-settings-grid">
                  <div className="app-settings-row">
                    <span>Version</span>
                    <strong>v{appInfo?.version ?? "1.0.0"}</strong>
                  </div>
                  <div className="app-settings-row">
                    <span>Status</span>
                    <strong>
                      {updateInfo
                        ? updateInfo.status === "unpublished"
                          ? "No release published yet"
                          : updateInfo.available
                            ? "Update available"
                            : "Up to date"
                        : "Not checked yet"}
                    </strong>
                  </div>
                  <div className="app-settings-row">
                    <span>Latest version</span>
                    <strong>{updateInfo?.latestVersion ? `v${updateInfo.latestVersion}` : updateInfo?.status === "unpublished" ? "No release yet" : "Not checked yet"}</strong>
                  </div>
                  <div className="app-settings-row">
                    <span>Package</span>
                    <strong>{updateInfo?.assetName || (updateInfo?.status === "unpublished" ? "Waiting for first release" : "GitHub release asset")}</strong>
                  </div>
                  <div className="app-settings-row">
                    <span>Download size</span>
                    <strong>{formatBytes(updateInfo?.assetSize ?? 0)}</strong>
                  </div>
                  <div className="app-settings-row">
                    <span>Published</span>
                    <strong>{formatTimestamp(updateInfo?.publishedAt ?? "")}</strong>
                  </div>
                  <div className="app-settings-row">
                    <span>Last checked</span>
                    <strong>{formatTimestamp(updateInfo?.checkedAt ?? "")}</strong>
                  </div>
                  <div className="app-settings-row">
                    <span>Update source</span>
                    <strong>GitHub Releases and Homebrew cask</strong>
                  </div>
                </div>
                <div className="app-update-actions">
                  <Button disabled={updateLoading} onClick={onCheckForUpdates} type="button" variant="ghost">
                    {updateLoading ? (
                      <>
                        <LoaderCircle className="is-spinning" size={14} />
                        <span>Checking...</span>
                      </>
                    ) : (
                      "Check for Updates"
                    )}
                  </Button>
                  {updateInfo?.available ? (
                    <Button onClick={onOpenUpdate} type="button" variant="primary">
                      Install Update
                    </Button>
                  ) : null}
                </div>
                <p className="settings-note">
                  {updateInfo?.status === "unpublished"
                    ? "No desktop release has been published yet. Once the first signed build is live, update metadata and install actions will show up here."
                    : updateInfo?.available
                      ? `mico ${updateInfo.latestVersion} is ready. Install opens the latest desktop download so you can replace this build cleanly.`
                      : "Check for updates whenever you want fresh release metadata. When a new build is available, install will appear here."}
                </p>
                <p className="settings-note settings-note--subtle">
                  Installs still hand off to the latest signed desktop release. The Homebrew cask remains the primary terminal-friendly install path.
                </p>
              </div>
            </div>
          ) : selectedTab === "theme" ? (
            <div className="preferences-panel__body">
              <div className="settings-section">
                <div className="settings-section__header">
                  <SunMoon size={15} />
                  <div>
                    <strong>Appearance</strong>
                    <span>Start with a restrained desktop theme and switch when you want.</span>
                  </div>
                </div>
                <div className="theme-toggle" data-nav-scope="theme-toggle" role="tablist" aria-label="Theme">
                  <button className={theme === THEMES.DARK ? "is-active" : ""} data-nav-item="true" onClick={() => onThemeChange(THEMES.DARK)} onKeyDown={(event) => handleScopedNavigation(event, "horizontal")} type="button">
                    Dark
                  </button>
                  <button className={theme === THEMES.LIGHT ? "is-active" : ""} data-nav-item="true" onClick={() => onThemeChange(THEMES.LIGHT)} onKeyDown={(event) => handleScopedNavigation(event, "horizontal")} type="button">
                    Light
                  </button>
                </div>
              </div>
            </div>
          ) : (
            <div className="preferences-panel__body">
              <div className="settings-section">
                <div className="settings-section__header">
                  <Gauge size={15} />
                  <div>
                    <strong>System dependencies</strong>
                    <span>Hover any status row to see what to do next.</span>
                  </div>
                </div>
                <div className="doctor-actions">
                  <Button onClick={onRefreshDoctor} type="button" variant="ghost">
                    Refresh doctor
                  </Button>
                </div>
                {doctorLoading ? <p>Checking your local setup...</p> : <DoctorChecklist checks={doctorReport?.checks ?? []} />}
              </div>
            </div>
          )}
        </div>
      </section>
    </div>
  );
}

function DoctorChecklist({ checks }: { checks: DoctorCheck[] }) {
  if (!checks.length) {
    return <p>No doctor data yet.</p>;
  }
  return (
    <div className="doctor-list">
      {checks.map((check) => (
        <article className={`doctor-item doctor-item--${check.status}`} key={check.name} title={check.help}>
          <div className="doctor-item__title">
            <span className={`doctor-item__dot doctor-item__dot--${check.status}`} />
            <strong>{check.name}</strong>
            {!check.required ? <em>Optional</em> : null}
          </div>
          <span>{check.detail}</span>
        </article>
      ))}
    </div>
  );
}
