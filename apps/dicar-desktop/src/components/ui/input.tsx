import type { InputHTMLAttributes } from "react";
import { cn } from "../../lib/cn";

export function Input({ className, ...props }: InputHTMLAttributes<HTMLInputElement>) {
  return <input className={cn("h-10 w-full rounded-[var(--radius)] border border-(--border) bg-(--background) px-3 font-mono text-sm text-(--text) placeholder:text-(--text-muted) disabled:cursor-not-allowed disabled:opacity-55", className)} {...props} />;
}
