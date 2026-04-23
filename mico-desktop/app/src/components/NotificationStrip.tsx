import type { Notification } from "../types";

interface NotificationStripProps {
  notifications: Notification[];
  onSeen: (id: string) => void;
}

export function NotificationStrip({ notifications, onSeen }: NotificationStripProps) {
  return (
    <section className="notifications" aria-label="Notifications">
      {notifications.slice(0, 4).map((notification) => (
        <button
          className={`notification notification--${notification.level} ${notification.seen ? "is-seen" : ""}`}
          key={notification.id}
          onClick={() => onSeen(notification.id)}
          type="button"
        >
          <strong>{notification.title}</strong>
          <span>{notification.body}</span>
        </button>
      ))}
    </section>
  );
}

