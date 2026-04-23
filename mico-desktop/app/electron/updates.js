const releaseURL = "https://github.com/thegeorgejoseph/mico/releases";
const latestReleaseAPI = "https://api.github.com/repos/thegeorgejoseph/mico/releases/latest";

function normalizeVersion(rawVersion) {
  return String(rawVersion || "")
    .trim()
    .replace(/^v/i, "")
    .split("-")[0];
}

function parseSemver(rawVersion) {
  const parts = normalizeVersion(rawVersion).split(".");
  return [parts[0], parts[1], parts[2]].map((part) => Number.parseInt(part || "0", 10) || 0);
}

function isVersionNewer(latestVersion, currentVersion) {
  const latest = parseSemver(latestVersion);
  const current = parseSemver(currentVersion);
  for (let index = 0; index < latest.length; index += 1) {
    if (latest[index] > current[index]) {
      return true;
    }
    if (latest[index] < current[index]) {
      return false;
    }
  }
  return false;
}

function pickReleaseAsset(assets) {
  const candidates = Array.isArray(assets) ? assets : [];
  const platformMatchers =
    process.platform === "darwin"
      ? [".dmg", ".zip"]
      : process.platform === "win32"
        ? [".exe", ".msi", ".zip"]
        : [".appimage", ".deb", ".tar.gz", ".zip"];

  for (const suffix of platformMatchers) {
    const match = candidates.find((asset) => String(asset?.name || "").toLowerCase().endsWith(suffix));
    if (match) {
      return match;
    }
  }
  return candidates[0] ?? null;
}

async function loadLatestRelease() {
  const response = await fetch(latestReleaseAPI, {
    headers: {
      Accept: "application/vnd.github+json",
      "User-Agent": "mico-desktop",
    },
  });
  if (response.status === 404) {
    return {
      assetName: "",
      assetSize: 0,
      checkedAt: new Date().toISOString(),
      currentVersion: "",
      downloadURL: releaseURL,
      error: "",
      latestVersion: "",
      publishedAt: "",
      releaseURL,
      status: "unpublished",
    };
  }
  if (!response.ok) {
    throw new Error(`Update check failed with ${response.status}`);
  }
  const payload = await response.json();
  const asset = pickReleaseAsset(payload.assets);
  return {
    assetName: asset?.name || "",
    assetSize: asset?.size || 0,
    checkedAt: new Date().toISOString(),
    currentVersion: "",
    downloadURL: asset?.browser_download_url || payload.html_url || releaseURL,
    error: "",
    latestVersion: normalizeVersion(payload.tag_name),
    publishedAt: payload.published_at || "",
    releaseURL: payload.html_url || releaseURL,
    status: "ready",
  };
}

module.exports = {
  isVersionNewer,
  loadLatestRelease,
  normalizeVersion,
  releaseURL,
};
