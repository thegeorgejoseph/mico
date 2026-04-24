import type { KeyboardEvent as ReactKeyboardEvent } from "react";

export type NavOrientation = "horizontal" | "vertical";

function focusSiblingInScope(current: HTMLElement, direction: number) {
  const scope = current.closest("[data-nav-scope]");
  if (!scope) {
    return;
  }
  const items = Array.from(scope.querySelectorAll<HTMLElement>('[data-nav-item="true"]:not(:disabled)'));
  const index = items.indexOf(current);
  if (index === -1 || items.length < 2) {
    return;
  }
  const next = items[(index + direction + items.length) % items.length];
  next.focus();
  if (next instanceof HTMLButtonElement) {
    next.click();
  }
}

export function handleScopedNavigation(event: ReactKeyboardEvent<HTMLElement>, orientation: NavOrientation = "vertical") {
  if (event.defaultPrevented || event.metaKey || event.ctrlKey || event.altKey) {
    return;
  }

  const forwardKeys = orientation === "horizontal" ? ["ArrowRight", "l"] : ["ArrowDown", "j"];
  const backwardKeys = orientation === "horizontal" ? ["ArrowLeft", "h"] : ["ArrowUp", "k"];

  if (forwardKeys.includes(event.key)) {
    event.preventDefault();
    focusSiblingInScope(event.currentTarget, 1);
    return;
  }

  if (backwardKeys.includes(event.key)) {
    event.preventDefault();
    focusSiblingInScope(event.currentTarget, -1);
  }
}
