import type { HTMLAttributes } from "react";
import { cn } from "../../lib/cn";

export function Badge({ className, ...props }: HTMLAttributes<HTMLSpanElement>) {
  return <span className={cn("inline-flex items-center gap-1 rounded-full border border-(--border) px-2 py-1 text-[11px] font-medium text-(--text-muted)", className)} {...props} />;
}
