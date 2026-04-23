import type { ButtonHTMLAttributes, KeyboardEvent, ReactNode } from "react";

interface ListItemProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  eyebrow?: string;
  icon?: ReactNode;
  meta?: string;
  selected?: boolean;
  title: string;
  children?: ReactNode;
}

export function ListItem({
  children,
  className = "",
  eyebrow,
  icon,
  meta,
  selected = false,
  title,
  onKeyDown,
  ...props
}: ListItemProps) {
  const selectedClass = selected ? "is-selected" : "";

  function focusSibling(current: HTMLButtonElement, direction: number) {
    const scope = current.closest("[data-nav-scope]");
    if (!scope) {
      return;
    }
    const items = Array.from(scope.querySelectorAll<HTMLButtonElement>('[data-nav-item="true"]:not(:disabled)'));
    const index = items.indexOf(current);
    if (index === -1 || items.length < 2) {
      return;
    }
    const next = items[(index + direction + items.length) % items.length];
    next.focus();
    next.click();
  }

  function handleKeyDown(event: KeyboardEvent<HTMLButtonElement>) {
    onKeyDown?.(event);
    if (event.defaultPrevented || event.metaKey || event.ctrlKey || event.altKey) {
      return;
    }
    if (event.key === "j" || event.key === "ArrowDown") {
      event.preventDefault();
      focusSibling(event.currentTarget, 1);
    }
    if (event.key === "k" || event.key === "ArrowUp") {
      event.preventDefault();
      focusSibling(event.currentTarget, -1);
    }
  }

  return (
    <button className={`list-item ${selectedClass} ${className}`.trim()} data-nav-item="true" onKeyDown={handleKeyDown} type="button" {...props}>
      {eyebrow ? <span className="list-item__eyebrow">{eyebrow}</span> : null}
      <div className="list-item__title">
        {icon ? <span className="list-item__icon">{icon}</span> : null}
        <strong>{title}</strong>
      </div>
      {children ? <span>{children}</span> : null}
      {meta ? <em>{meta}</em> : null}
    </button>
  );
}
