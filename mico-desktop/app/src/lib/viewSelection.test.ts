import assert from "node:assert/strict";
import test from "node:test";

import { deriveViewSelection } from "./viewSelection.ts";
import type { MicoState } from "../types";

function createState(overrides: Partial<MicoState>): MicoState {
  return {
    version: 1,
    repos: [],
    worktrees: [],
    sessions: [],
    notifications: [],
    selection: { repoId: "", worktreeId: "", sessionId: "", mode: "effort" },
    logs: [],
    ...overrides,
  };
}

test("deriveViewSelection keeps a selected project without forcing another project's worktree", () => {
  const state = createState({
    repos: [
      { id: "repo-rustbook", name: "rustbook", path: "/tmp/rustbook", createdAt: "2026-04-22T00:00:00Z" },
      { id: "repo-other", name: "other", path: "/tmp/other", createdAt: "2026-04-22T00:00:00Z" },
    ],
    worktrees: [
      {
        id: "wt-other",
        repoId: "repo-other",
        branch: "main",
        base: "main",
        path: "/tmp/other-main",
        status: "running",
        createdAt: "2026-04-22T00:00:00Z",
        updatedAt: "2026-04-22T00:00:00Z",
      },
    ],
    sessions: [
      {
        id: "ses-other",
        worktreeId: "wt-other",
        agent: "terminal",
        command: ["/bin/zsh"],
        sessionName: "legacy-session",
        status: "running",
        createdAt: "2026-04-22T00:00:00Z",
        updatedAt: "2026-04-22T00:00:00Z",
      },
    ],
    selection: { repoId: "repo-rustbook", worktreeId: "", sessionId: "", mode: "effort" },
  });

  assert.equal(
    JSON.stringify(deriveViewSelection(state)),
    JSON.stringify({
      repoId: "repo-rustbook",
      worktreeId: "",
      sessionId: "",
    }),
  );
});

test("deriveViewSelection clears stale worktree and session that do not belong to the selected project", () => {
  const state = createState({
    repos: [
      { id: "repo-rustbook", name: "rustbook", path: "/tmp/rustbook", createdAt: "2026-04-22T00:00:00Z" },
      { id: "repo-other", name: "other", path: "/tmp/other", createdAt: "2026-04-22T00:00:00Z" },
    ],
    worktrees: [
      {
        id: "wt-other",
        repoId: "repo-other",
        branch: "main",
        base: "main",
        path: "/tmp/other-main",
        status: "running",
        createdAt: "2026-04-22T00:00:00Z",
        updatedAt: "2026-04-22T00:00:00Z",
      },
    ],
    sessions: [
      {
        id: "ses-other",
        worktreeId: "wt-other",
        agent: "terminal",
        command: ["/bin/zsh"],
        sessionName: "legacy-session",
        status: "running",
        createdAt: "2026-04-22T00:00:00Z",
        updatedAt: "2026-04-22T00:00:00Z",
      },
    ],
    selection: { repoId: "repo-rustbook", worktreeId: "wt-other", sessionId: "ses-other", mode: "effort" },
  });

  assert.equal(
    JSON.stringify(deriveViewSelection(state)),
    JSON.stringify({
      repoId: "repo-rustbook",
      worktreeId: "",
      sessionId: "",
    }),
  );
});

test("deriveViewSelection falls back to the first project when selection is empty", () => {
  const state = createState({
    repos: [
      { id: "repo-rustbook", name: "rustbook", path: "/tmp/rustbook", createdAt: "2026-04-22T00:00:00Z" },
      { id: "repo-other", name: "other", path: "/tmp/other", createdAt: "2026-04-22T00:00:00Z" },
    ],
  });

  assert.equal(
    JSON.stringify(deriveViewSelection(state)),
    JSON.stringify({
      repoId: "repo-rustbook",
      worktreeId: "",
      sessionId: "",
    }),
  );
});
