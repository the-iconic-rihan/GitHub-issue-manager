import { createContext, useContext, type ReactNode } from "react";

import { useApi } from "@/lib/useApi";
import type { Me } from "@/lib/types";

export interface MeContextValue {
  me?: Me;
  loading: boolean;
  error?: Error;
  reload: () => void;
}

export const MeContext = createContext<MeContextValue>({ loading: true, reload: () => undefined });

/**
 * Loads the signed-in user from the backend once per session (mounted in `_app`, so it
 * survives page navigation). The backend call also records the user ("connects" the account).
 */
export function MeProvider({ children }: { children: ReactNode }) {
  const { data, loading, error, reload } = useApi<Me>("me");
  return <MeContext.Provider value={{ me: data, loading, error, reload }}>{children}</MeContext.Provider>;
}

export function useMe() {
  return useContext(MeContext);
}
