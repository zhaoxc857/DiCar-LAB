import type { ButtonHTMLAttributes } from "react";
import { cn } from "../../lib/cn";

type ButtonProps = ButtonHTMLAttributes<HTMLButtonElement> & {
  variant?: "primary" | "secondary" | "danger";
  size?: "sm" | "md";
};

export function Button({ className, variant = "primary", size = "md", ...props }: ButtonProps) {
  return (
    <button
      className={cn(
        "inline-flex items-center justify-center gap-2 rounded-[var(--radius)] border font-semibold transition-colors disabled:cursor-not-allowed disabled:opacity-50",
        size === "sm" ? "min-h-8 px-3 text-xs" : "min-h-10 px-4 text-sm",
        variant === "primary" && "border-(--interactive) bg-(--interactive) text-(--background) hover:bg-(--interactive-strong)",
        variant === "secondary" && "border-(--border) bg-(--surface-raised) text-(--text) hover:border-(--interactive)",
        variant === "danger" && "border-(--danger) bg-transparent text-(--danger) hover:bg-[color-mix(in_srgb,var(--danger)_12%,transparent)]",
        className,
      )}
      {...props}
    />
  );
}
