import type { PropsWithChildren } from "react";

export function Alert({ children }: PropsWithChildren) {
  return <div role="alert" className="rounded-[var(--radius)] border border-(--danger) bg-[color-mix(in_srgb,var(--danger)_8%,transparent)] px-3 py-2 text-sm text-(--danger)">{children}</div>;
}
