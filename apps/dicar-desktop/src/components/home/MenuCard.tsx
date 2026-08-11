import type { Icon } from "@phosphor-icons/react";
import { ArrowRight } from "@phosphor-icons/react";
import { Link } from "react-router";
import { Badge } from "../ui/badge";
import { Card } from "../ui/card";

type MenuCardProps = {
  to: string;
  title: string;
  description: string;
  status: "可用" | "计划发布";
  icon: Icon;
};

export function MenuCard({ to, title, description, status, icon: MenuIcon }: MenuCardProps) {
  return (
    <Link className="group no-underline" to={to}>
      <Card className="h-full min-h-44 p-5 transition duration-150 hover:-translate-y-0.5 hover:border-(--interactive) hover:bg-(--surface-hover)">
        <div className="flex items-start justify-between gap-4">
          <span className="grid size-10 place-items-center rounded-[var(--radius)] border border-(--border) bg-(--surface) text-(--interactive)"><MenuIcon aria-hidden="true" size={23} weight="duotone" /></span>
          <Badge className={status === "可用" ? "border-(--success) text-(--success)" : "border-(--warning) text-(--warning)"}>{status}</Badge>
        </div>
        <h2 className="mb-0 mt-5 text-base font-semibold">{title}</h2>
        <p className="mb-0 mt-2 max-w-md text-sm leading-6 text-(--text-muted)">{description}</p>
        <span className="mt-4 inline-flex items-center gap-1 text-xs font-semibold text-(--interactive)">打开功能 <ArrowRight aria-hidden="true" className="transition-transform group-hover:translate-x-1" size={14} /></span>
      </Card>
    </Link>
  );
}
