import type { ReactNode } from "react";

export function PermissionGate({ children, deniedReason }: { children: ReactNode; deniedReason: string | null }) {
  return <div aria-disabled={deniedReason !== null}>{children}{deniedReason && <p className="m-0 mt-2 text-xs text-(--warning)">{deniedReason}</p>}</div>;
}
