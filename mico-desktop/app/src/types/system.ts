export type DoctorStatus = "ok" | "warning" | "error";

export interface DoctorCheck {
  name: string;
  status: DoctorStatus;
  detail: string;
  help: string;
  required: boolean;
}

export interface DoctorReport {
  checks: DoctorCheck[];
}

export interface AppInfo {
  name: string;
  packaged: boolean;
  releaseURL: string;
  version: string;
}

export interface UpdateInfo {
  assetName: string;
  assetSize: number;
  available: boolean;
  checkedAt: string;
  currentVersion: string;
  downloadURL: string;
  error: string;
  latestVersion: string;
  publishedAt: string;
  releaseURL: string;
  status: "ready" | "unpublished";
}
