import { spawn } from "node:child_process";
import fs from "node:fs/promises";
import net from "node:net";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

import { createServer } from "vite";

const appDir = path.dirname(fileURLToPath(import.meta.url));
const desktopDir = path.dirname(appDir);
const backendDir = path.join(desktopDir, "backend");
const backendBinDir = path.join(backendDir, "bin");
const backendBinaryName = process.platform === "win32" ? "mico-desktop-dev.exe" : "mico-desktop-dev";
const backendBinaryPath = path.join(backendBinDir, backendBinaryName);
const npmCommand = process.platform === "win32" ? "npm.cmd" : "npm";
const goCommand = process.platform === "win32" ? "go.exe" : "go";

let viteServer;
let electronProcess;
let cleaningUp = false;

function reserveLoopbackPort() {
  return new Promise((resolve, reject) => {
    const server = net.createServer();

    server.on("error", (error) => {
      reject(error);
    });

    server.listen(0, "127.0.0.1", () => {
      const address = server.address();
      if (!address || typeof address === "string") {
        server.close(() => {
          reject(new Error("Unable to reserve renderer dev server port"));
        });
        return;
      }

      const { port } = address;
      server.close((error) => {
        if (error) {
          reject(error);
          return;
        }
        resolve(port);
      });
    });
  });
}

function rendererURLFromServer(server) {
  const localURL = server.resolvedUrls?.local?.[0];
  if (localURL) {
    return localURL.replace(/\/$/, "");
  }

  const address = server.httpServer?.address();
  if (!address || typeof address === "string") {
    throw new Error("Unable to determine renderer dev server address");
  }

  return `http://127.0.0.1:${address.port}`;
}

function runCommand(command, args, options = {}) {
  return new Promise((resolve, reject) => {
    const child = spawn(command, args, {
      stdio: "inherit",
      ...options,
    });

    child.on("error", (error) => {
      reject(new Error(`Unable to start ${command}: ${error.message}`));
    });

    child.on("exit", (code, signal) => {
      if (signal) {
        reject(new Error(`${command} exited from signal ${signal}`));
        return;
      }
      if (code !== 0) {
        reject(new Error(`${command} exited with status ${code ?? "unknown"}`));
        return;
      }
      resolve();
    });
  });
}

async function buildBackendBinary() {
  await fs.mkdir(backendBinDir, { recursive: true });
  await runCommand(goCommand, ["build", "-o", backendBinaryPath, "./cmd/mico-desktop"], {
    cwd: backendDir,
  });
  return backendBinaryPath;
}

async function cleanup(exitCode = 0) {
  if (cleaningUp) {
    return;
  }
  cleaningUp = true;

  if (electronProcess && !electronProcess.killed) {
    electronProcess.kill("SIGTERM");
  }

  if (viteServer) {
    await viteServer.close();
  }

  process.exit(exitCode);
}

async function main() {
  const compiledBackendPath = await buildBackendBinary();
  const rendererPort = await reserveLoopbackPort();
  viteServer = await createServer({
    configFile: path.join(appDir, "vite.config.ts"),
    server: {
      host: "127.0.0.1",
      port: rendererPort,
      strictPort: true,
    },
  });

  await viteServer.listen(rendererPort);
  viteServer.printUrls();

  const rendererURL = rendererURLFromServer(viteServer);
  electronProcess = spawn(npmCommand, ["run", "dev:electron"], {
    cwd: appDir,
    env: {
      ...process.env,
      MICO_BACKEND_BIN: compiledBackendPath,
      MICO_RENDERER_URL: rendererURL,
    },
    stdio: "inherit",
  });

  electronProcess.on("exit", async (code, signal) => {
    if (signal) {
      await cleanup(1);
      return;
    }
    await cleanup(code ?? 0);
  });
}

process.on("SIGINT", () => {
  void cleanup(0);
});

process.on("SIGTERM", () => {
  void cleanup(0);
});

main().catch(async (error) => {
  console.error(error);
  await cleanup(1);
});
