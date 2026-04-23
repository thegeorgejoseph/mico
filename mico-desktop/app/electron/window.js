const path = require("node:path");
const { BrowserWindow } = require("electron");

function resolveRendererURL() {
  const rendererArg = process.argv.find((value) => value.startsWith("--mico-renderer-url="));
  if (rendererArg) {
    return rendererArg.slice("--mico-renderer-url=".length);
  }
  return process.env.MICO_RENDERER_URL || "";
}

function createWindow({ app, appIconPath, backendOrigin, preloadPath }) {
  const win = new BrowserWindow({
    width: 1320,
    height: 860,
    minWidth: 980,
    minHeight: 640,
    title: "mico",
    backgroundColor: "#17191e",
    titleBarStyle: process.platform === "darwin" ? "hiddenInset" : "default",
    vibrancy: process.platform === "darwin" ? "sidebar" : undefined,
    visualEffectState: process.platform === "darwin" ? "followWindow" : undefined,
    trafficLightPosition: process.platform === "darwin" ? { x: 16, y: 18 } : undefined,
    icon: process.platform === "darwin" ? undefined : appIconPath,
    webPreferences: {
      additionalArguments: [`--mico-backend-origin=${backendOrigin}`],
      preload: preloadPath,
      contextIsolation: true,
      nodeIntegration: false,
    },
  });

  const rendererURL = resolveRendererURL();
  if (rendererURL) {
    win.loadURL(rendererURL);
  } else {
    win.loadFile(path.join(path.dirname(preloadPath), "dist", "index.html"));
  }

  if (process.platform === "darwin") {
    win.setWindowButtonVisibility(true);
  }

  return win;
}

module.exports = {
  createWindow,
};
