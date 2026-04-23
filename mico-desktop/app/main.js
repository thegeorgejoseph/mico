const { app, dialog, ipcMain, nativeImage, shell } = require("electron");
const fs = require("node:fs");
const path = require("node:path");

const { startBackend } = require("./electron/backend");
const { registerIpcHandlers } = require("./electron/ipc");
const updates = require("./electron/updates");
const { createWindow } = require("./electron/window");

let backendProcess;
let mainWindow;

const appIconPath = path.join(__dirname, "assets", "mico-shell-icon.png");
const appIconICNSPath = path.join(__dirname, "assets", "mico-shell-icon.icns");

function isDevBundleLaunch() {
  return process.argv.includes("--mico-dev-bundle");
}

function showDesktopError(title, detail) {
  const message = detail instanceof Error ? detail.message : String(detail || "");
  process.stderr.write(`[desktop] ${title}: ${message}\n`);
  if (app.isReady()) {
    dialog.showErrorBox(title, message);
    return;
  }
  console.error(title, message);
}

app.setName("mico");

process.on("uncaughtException", (error) => {
  showDesktopError("mico main process error", error);
});

process.on("unhandledRejection", (reason) => {
  showDesktopError("mico unhandled rejection", reason);
});

app.whenReady().then(async () => {
  if (process.platform === "darwin") {
    const dockIconPath = fs.existsSync(appIconICNSPath) ? appIconICNSPath : appIconPath;
    const appIcon = nativeImage.createFromPath(dockIconPath);
    if (!appIcon.isEmpty()) {
      app.dock.setIcon(appIcon);
    }
  }

  registerIpcHandlers({
    app,
    dialog,
    ipcMain,
    releaseURL: updates.releaseURL,
    shell,
    updates,
  });

  let backendOrigin;
  try {
    const backend = await startBackend({
      app,
      isDevBundleLaunch: isDevBundleLaunch(),
    });
    backendOrigin = backend.origin;
    backendProcess = backend.processHandle;
  } catch (error) {
    process.stderr.write(`[backend] failed to start: ${error.message}\n`);
    dialog.showErrorBox("mico failed to start", `The local backend could not be started.\n\n${error.message}`);
    app.exit(1);
    return;
  }

  mainWindow = createWindow({
    app,
    appIconPath,
    backendOrigin,
    preloadPath: path.join(__dirname, "preload.js"),
  });

  mainWindow.on("closed", () => {
    if (mainWindow) {
      mainWindow = null;
    }
  });

  mainWindow.webContents.on("render-process-gone", (_event, details) => {
    process.stderr.write(`[renderer] process gone: reason=${details.reason} exitCode=${details.exitCode ?? "null"}\n`);
    dialog
      .showMessageBox(mainWindow, {
        type: "error",
        buttons: ["Reload", "Close"],
        defaultId: 0,
        cancelId: 1,
        title: "mico renderer stopped",
        message: "The renderer process stopped unexpectedly.",
        detail: `Reason: ${details.reason}${details.exitCode != null ? `\nExit code: ${details.exitCode}` : ""}`,
      })
      .then(({ response }) => {
        if (!mainWindow?.isDestroyed()) {
          if (response === 0) {
            mainWindow.reload();
            return;
          }
          mainWindow.close();
        }
      })
      .catch(() => {
        if (!mainWindow?.isDestroyed()) {
          mainWindow.close();
        }
      });
  });

  app.on("activate", () => {
    if (mainWindow?.isDestroyed()) {
      mainWindow = null;
    }
    if (!mainWindow) {
      mainWindow = createWindow({
        app,
        appIconPath,
        backendOrigin,
        preloadPath: path.join(__dirname, "preload.js"),
      });
    }
  });
});

app.on("window-all-closed", () => {
  if (process.platform !== "darwin") {
    app.quit();
  }
});

app.on("before-quit", () => {
  if (backendProcess && !backendProcess.killed) {
    backendProcess.kill();
  }
});
