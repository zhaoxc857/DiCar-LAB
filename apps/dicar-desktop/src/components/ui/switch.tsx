import type { InputHTMLAttributes } from "react";

export function Switch(props: Omit<InputHTMLAttributes<HTMLInputElement>, "type" | "role">) {
  return <input className="size-5 accent-(--interactive)" role="switch" type="checkbox" {...props} />;
}
