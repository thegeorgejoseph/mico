function registerIpcHandlers({ app, dialog, ipcMain, releaseURL, shell, updates }) {
  ipcMain.handle("mico:pick-repo-folder", async () => {
    const result = await dialog.showOpenDialog({
      buttonLabel: "Add Project",
      properties: ["openDirectory"],
      title: "Choose a Git Project",
    });
    if (result.canceled || result.filePaths.length === 0) {
      return null;
    }
    return result.filePaths[0];
  });

  ipcMain.handle("mico:app-info", async () => ({
    name: app.getName(),
    packaged: app.isPackaged,
    releaseURL,
    version: app.getVersion(),
  }));

  ipcMain.handle("mico:check-for-updates", async () => {
    const currentVersion = updates.normalizeVersion(app.getVersion());
    const latestRelease = await updates.loadLatestRelease();
    return {
      ...latestRelease,
      available: latestRelease.status === "ready" && updates.isVersionNewer(latestRelease.latestVersion, currentVersion),
      currentVersion,
    };
  });

  ipcMain.handle("mico:open-update", async (_event, targetURL) => {
    await shell.openExternal(targetURL || releaseURL);
    return { ok: true };
  });
}

module.exports = {
  registerIpcHandlers,
};
