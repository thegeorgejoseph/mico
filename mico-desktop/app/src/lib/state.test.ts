import assert from "node:assert/strict";
import test from "node:test";

import { normalizeMicoState, normalizeStringList } from "./state.ts";

test("normalizeMicoState turns null collections into safe empty arrays", () => {
  const normalized = normalizeMicoState({
    version: 7,
    repos: null,
    worktrees: null,
    sessions: null,
    notifications: null,
    selection: null,
    logs: null,
    migrations: null,
  });

  assert.equal(normalized.version, 7);
  assert.equal(JSON.stringify(normalized.repos), JSON.stringify([]));
  assert.equal(JSON.stringify(normalized.worktrees), JSON.stringify([]));
  assert.equal(JSON.stringify(normalized.sessions), JSON.stringify([]));
  assert.equal(JSON.stringify(normalized.notifications), JSON.stringify([]));
  assert.equal(JSON.stringify(normalized.logs), JSON.stringify([]));
  assert.equal(JSON.stringify(normalized.migrations), JSON.stringify([]));
  assert.equal(JSON.stringify(normalized.selection), JSON.stringify({
    repoId: "",
    worktreeId: "",
    sessionId: "",
    mode: "effort",
  }));
});

test("normalizeMicoState preserves valid data and normalizes selection mode", () => {
  const normalized = normalizeMicoState({
    version: 3,
    repos: [{ id: "repo-1", name: "mico", path: "/tmp/mico", createdAt: "2026-04-24T00:00:00Z" }],
    worktrees: [{ id: "wt-1", repoId: "repo-1", branch: "main", base: "main", path: "/tmp/mico-main", status: "running", createdAt: "2026-04-24T00:00:00Z", updatedAt: "2026-04-24T00:00:00Z" }],
    sessions: [{ id: "ses-1", worktreeId: "wt-1", agent: "codex", command: ["codex"], sessionName: "mico-desktop-wt-1-codex", status: "running", createdAt: "2026-04-24T00:00:00Z", updatedAt: "2026-04-24T00:00:00Z" }],
    notifications: [{ id: "ntf-1", level: "info", title: "Heads up", body: "Still working", seen: false, createdAt: "2026-04-24T00:00:00Z" }],
    selection: { repoId: "repo-1", worktreeId: "wt-1", sessionId: "ses-1", mode: "agent" },
    logs: [{ id: "log-1", level: "info", scope: "app", action: "load", message: "ready", createdAt: "2026-04-24T00:00:00Z" }],
    migrations: [{ id: "20260424", appliedAt: "2026-04-24T00:00:00Z" }],
  });

  assert.equal(normalized.selection.mode, "agent");
  assert.equal(normalized.repos[0]?.id, "repo-1");
  assert.equal(normalized.worktrees[0]?.id, "wt-1");
  assert.equal(normalized.sessions[0]?.id, "ses-1");
  assert.equal(normalized.notifications[0]?.id, "ntf-1");
  assert.equal(normalized.logs[0]?.id, "log-1");
  assert.equal(normalized.migrations?.[0]?.id, "20260424");
});

test("normalizeStringList filters out non-string branch payloads", () => {
  assert.equal(JSON.stringify(normalizeStringList(["main", 7, null, "feature"])), JSON.stringify(["main", "feature"]));
  assert.equal(JSON.stringify(normalizeStringList(null)), JSON.stringify([]));
});
