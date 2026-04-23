import test from "node:test";
import assert from "node:assert/strict";

import { compareSessions, isDesktopManagedSession } from "./sessionSelection.ts";

test("isDesktopManagedSession identifies desktop-owned sessions", () => {
  assert.equal(
    isDesktopManagedSession({
      id: "desktop",
      worktreeId: "wt-1",
      agent: "codex",
      command: ["codex"],
      sessionName: "mico-desktop-wt-1-codex",
      status: "running",
      createdAt: "2026-01-01T00:00:00Z",
      updatedAt: "2026-01-01T00:00:00Z",
    }),
    true,
  );
});

test("compareSessions prefers desktop-managed sessions over legacy ones", () => {
  const legacy = {
    id: "legacy",
    worktreeId: "wt-1",
    agent: "terminal" as const,
    command: ["/bin/zsh"],
    sessionName: "legacy-session",
    status: "running" as const,
    createdAt: "2026-01-01T00:00:00Z",
    updatedAt: "2026-01-01T00:00:00Z",
  };
  const desktop = {
    id: "desktop",
    worktreeId: "wt-1",
    agent: "codex" as const,
    command: ["codex"],
    sessionName: "mico-desktop-wt-1-codex",
    status: "running" as const,
    createdAt: "2026-01-01T00:00:00Z",
    updatedAt: "2026-01-01T00:00:01Z",
  };

  assert.ok(compareSessions(desktop, legacy) < 0);
});
