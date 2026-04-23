const fs = require("node:fs");
const net = require("node:net");
const path = require("node:path");
const { spawn } = require("node:child_process");

function reserveLoopbackPort() {
  return new Promise((resolve, reject) => {
    const server = net.createServer();
    server.unref();
    server.on("error", reject);
    server.listen(0, "127.0.0.1", () => {
      const address = server.address();
      if (!address || typeof address === "string") {
        server.close(() => reject(new Error("Unable to reserve a loopback port")));
        return;
      }
      server.close((error) => {
        if (error) {
          reject(error);
          return;
        }
        resolve(address.port);
      });
    });
  });
}

async function waitForBackend(origin, timeoutMs = 15000) {
  const startedAt = Date.now();
  while (Date.now() - startedAt < timeoutMs) {
    try {
      const response = await fetch(`${origin}/api/health`);
      if (response.ok) {
        return;
      }
    } catch {
      // Keep polling until the backend is ready or times out.
    }
    await new Promise((resolve) => setTimeout(resolve, 250));
  }
  throw new Error(`Backend did not become ready at ${origin}`);
}

function resolveBackendLaunch({ app, port, isDevBundleLaunch }) {
  if (process.env.MICO_BACKEND_BIN) {
    return {
      args: ["--addr", `127.0.0.1:${port}`],
      command: process.env.MICO_BACKEND_BIN,
    };
  }

  if (app.isPackaged && !isDevBundleLaunch) {
    const binaryName = process.platform === "win32" ? "mico-desktop.exe" : "mico-desktop";
    const packagedCandidates = [
      path.join(process.resourcesPath, "backend", binaryName),
      path.join(process.resourcesPath, binaryName),
    ];
    const binaryPath = packagedCandidates.find((candidate) => fs.existsSync(candidate));
    if (!binaryPath) {
      throw new Error(`Packaged backend binary not found. Expected one of: ${packagedCandidates.join(", ")}`);
    }
    return {
      args: ["--addr", `127.0.0.1:${port}`],
      command: binaryPath,
    };
  }

  return {
    args: ["run", "./cmd/mico-desktop", "--addr", `127.0.0.1:${port}`],
    command: "go",
    cwd: path.join(__dirname, "..", "backend"),
  };
}

async function startBackend({ app, isDevBundleLaunch }) {
  const port = await reserveLoopbackPort();
  const origin = `http://127.0.0.1:${port}`;
  const backend = resolveBackendLaunch({ app, isDevBundleLaunch, port });
  const processHandle = spawn(backend.command, backend.args, {
    cwd: backend.cwd,
    stdio: ["ignore", "pipe", "pipe"],
  });

  const readiness = new Promise((resolve, reject) => {
    let settled = false;
    const settle = (callback) => (value) => {
      if (settled) {
        return;
      }
      settled = true;
      callback(value);
    };

    const succeed = settle(resolve);
    const fail = settle(reject);

    processHandle.once("error", (error) => {
      const location = backend.cwd ? ` (cwd ${backend.cwd})` : "";
      fail(new Error(`Unable to start backend command ${backend.command}${location}: ${error.message}`));
    });
    processHandle.once("exit", (code, signal) => {
      if (code === 0 && !signal) {
        return;
      }
      const detail = signal ? `signal ${signal}` : `code ${code ?? "unknown"}`;
      fail(new Error(`Backend process exited before becoming ready (${detail})`));
    });

    waitForBackend(origin)
      .then(() => {
        succeed();
      })
      .catch((error) => {
        fail(error);
      });
  });

  processHandle.stdout.on("data", (chunk) => {
    process.stdout.write(`[backend] ${chunk}`);
  });
  processHandle.stderr.on("data", (chunk) => {
    process.stderr.write(`[backend] ${chunk}`);
  });
  processHandle.on("exit", (code, signal) => {
    process.stderr.write(`[backend] exited code=${code ?? "null"} signal=${signal ?? "null"}\n`);
  });

  await readiness;
  return { origin, processHandle };
}

module.exports = {
  startBackend,
};
