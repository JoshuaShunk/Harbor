import { createContext, useContext } from "react";
import { useUpdater, type UseUpdaterReturn } from "../hooks/useUpdater";

const UpdateContext = createContext<UseUpdaterReturn | null>(null);

export function UpdateProvider({ children }: { children: React.ReactNode }) {
  const updater = useUpdater(true);
  return (
    <UpdateContext.Provider value={updater}>{children}</UpdateContext.Provider>
  );
}

export function useUpdate(): UseUpdaterReturn {
  const ctx = useContext(UpdateContext);
  if (!ctx) throw new Error("useUpdate must be used within UpdateProvider");
  return ctx;
}
