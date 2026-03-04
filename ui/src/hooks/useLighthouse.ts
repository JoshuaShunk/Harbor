import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { gatewayStatus, startGateway, stopGateway } from "../lib/tauri";

export function useLighthouse() {
  const [running, setRunning] = useState(false);
  const [toggling, setToggling] = useState(false);

  useEffect(() => {
    gatewayStatus().then(setRunning).catch(() => setRunning(false));

    const unlisten = listen<boolean>("gateway-status-changed", (e) => {
      setRunning(e.payload);
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  async function toggle() {
    if (toggling) return;
    setToggling(true);
    try {
      if (running) {
        await stopGateway();
      } else {
        await startGateway();
      }
    } finally {
      setToggling(false);
    }
  }

  return { running, toggling, toggle };
}
