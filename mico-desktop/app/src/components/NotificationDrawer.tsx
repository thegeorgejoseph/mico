import { Bell, FolderGit2, GitPullRequestArrow, Rocket, SquareTerminal, X } from "lucide-react";

import type { Notification } from "../types";

interface NotificationDrawerProps {
  notifications: Notification[];
  onClose: () => void;
  onSeen: (id: string) => void;
  open: boolean;
}

export function NotificationDrawer({ notifications, onClose, onSeen, open }: NotificationDrawerProps) {
  if (!open) {
    return null;
  }
  const visibleNotifications = notifications.filter((notification) => !notification.seen);

  return (
    <div className="side-sheet-backdrop" role="presentation" onMouseDown={onClose}>
      <aside className="side-sheet" onMouseDown={(event) => event.stopPropagation()}>
        <div className="side-sheet__header">
          <div>
            <h2>Notifications</h2>
            <p>Recent mico activity that wants your attention.</p>
          </div>
          <button className="toolbar-icon" onClick={onClose} type="button" aria-label="Close notifications" title="Close notifications">
            <X size={16} />
          </button>
        </div>
        <div className="side-sheet__body">
          {visibleNotifications.length ? (
            visibleNotifications.map((notification) => (
              <article className={`drawer-notification drawer-notification--${notification.level}`} key={notification.id}>
                <div>
                  <div className="drawer-notification__title">
                    <span className="drawer-notification__icon">{notificationIcon(notification)}</span>
                    <strong>{notification.title}</strong>
                  </div>
                  <span>{notification.body}</span>
                </div>
                <button className="drawer-notification__dismiss" onClick={() => void onSeen(notification.id)} type="button" aria-label={`Dismiss ${notification.title}`} title={`Dismiss ${notification.title}`}>
                  <X size={12} />
                </button>
              </article>
            ))
          ) : (
            <div className="drawer-empty">
              <Bell size={18} />
              <p>No notifications right now.</p>
            </div>
          )}
        </div>
      </aside>
    </div>
  );
}

function notificationIcon(notification: Notification) {
  if (notification.title === "Worktree created") {
    return <Rocket size={14} />;
  }
  if (notification.title === "Project added") {
    return <FolderGit2 size={14} />;
  }
  if (notification.title === "Session started") {
    return <SquareTerminal size={14} />;
  }

  switch (notification.level) {
    case "success":
      return <Rocket size={14} />;
    case "warning":
      return <GitPullRequestArrow size={14} />;
    case "error":
      return <X size={14} />;
    default:
      return <Bell size={14} />;
  }
}
