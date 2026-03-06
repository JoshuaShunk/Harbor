import { useState, useEffect, useCallback, useRef } from "react";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { getVersion } from "@tauri-apps/api/app";
export type UpdateStatus =
  | "idle"
  | "checking"
  | "up-to-date"
  | "available"
  | "downloading"
  | "ready"
  | "error";

export interface UpdateProgress {
  downloaded: number;
  total: number;
}

export interface UseUpdaterReturn {
  status: UpdateStatus;
  currentVersion: string;
  availableVersion: string | null;
  progress: UpdateProgress | null;
  error: string | null;
  checkForUpdate: () => Promise<void>;
  downloadAndInstall: () => Promise<void>;
}

export function useUpdater(checkOnMount = false): UseUpdaterReturn {
  const [status, setStatus] = useState<UpdateStatus>("idle");
  const [currentVersion, setCurrentVersion] = useState("0.1.0");
  const [availableVersion, setAvailableVersion] = useState<string | null>(null);
  const [progress, setProgress] = useState<UpdateProgress | null>(null);
  const [error, setError] = useState<string | null>(null);
  const updateRef = useRef<Update | null>(null);

  useEffect(() => {
    getVersion().then(setCurrentVersion).catch(() => {});
  }, []);

  const checkForUpdate = useCallback(async () => {
    setStatus("checking");
    setError(null);
    try {
      const update = await check();
      if (update?.available) {
        updateRef.current = update;
        setAvailableVersion(update.version);
        setStatus("available");
      } else {
        updateRef.current = null;
        setAvailableVersion(null);
        setStatus("up-to-date");
      }
    } catch (e) {
      setStatus("error");
      setError(String(e));
    }
  }, []);

  const downloadAndInstall = useCallback(async () => {
    const update = updateRef.current;
    if (!update) return;

    setStatus("downloading");
    setProgress({ downloaded: 0, total: 0 });

    try {
      await update.downloadAndInstall((event) => {
        if (event.event === "Started" && event.data.contentLength) {
          setProgress({ downloaded: 0, total: event.data.contentLength });
        } else if (event.event === "Progress") {
          setProgress((prev) =>
            prev
              ? { ...prev, downloaded: prev.downloaded + (event.data.chunkLength ?? 0) }
              : null,
          );
        } else if (event.event === "Finished") {
          setStatus("ready");
        }
      });
      await relaunch();
    } catch (e) {
      setStatus("error");
      setError(String(e));
    }
  }, []);

  useEffect(() => {
    if (checkOnMount) {
      const timer = setTimeout(() => checkForUpdate(), 2000);
      const interval = setInterval(() => checkForUpdate(), 60 * 60 * 1000);
      return () => {
        clearTimeout(timer);
        clearInterval(interval);
      };
    }
  }, [checkOnMount, checkForUpdate]);

  return {
    status,
    currentVersion,
    availableVersion,
    progress,
    error,
    checkForUpdate,
    downloadAndInstall,
  };
}
