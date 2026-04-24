import type { Session } from "../types";

export function isDesktopManagedSession(session: Session) {
  return session.sessionName.startsWith("mico-desktop-");
}

export function compareSessions(left: Session, right: Session) {
  const leftManaged = isDesktopManagedSession(left) ? 0 : 1;
  const rightManaged = isDesktopManagedSession(right) ? 0 : 1;
  if (leftManaged !== rightManaged) {
    return leftManaged - rightManaged;
  }
  return new Date(right.updatedAt).getTime() - new Date(left.updatedAt).getTime();
}
