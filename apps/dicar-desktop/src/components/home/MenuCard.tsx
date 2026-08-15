import type { Icon } from "@phosphor-icons/react";
import { ArrowRight } from "@phosphor-icons/react";
import { Link } from "react-router";
import { Card } from "../ui/card";

type MenuCardProps = {
  to: string;
  title: string;
  description: string;
  actionLabel: string;
  icon: Icon;
};

export function MenuCard({ to, title, description, actionLabel, icon: MenuIcon }: MenuCardProps) {
  return (
    <Link aria-label={actionLabel} className="group no-underline" to={to}>
      <Card className="h-full min-h-44 p-5 transition duration-150 hover:-translate-y-0.5 hover:border-(--interactive) hover:bg-(--surface-hover)">
        <span className="grid size-10 place-items-center rounded-[var(--radius)] border border-(--border) bg-(--surface) text-(--interactive)"><MenuIcon aria-hidden="true" size={23} weight="duotone" /></span>
        <h2 className="mb-0 mt-5 text-base font-semibold">{title}</h2>
        <p className="mb-0 mt-2 max-w-md text-sm leading-6 text-(--text-muted)">{description}</p>
        <span className="mt-4 inline-flex items-center gap-1 text-xs font-semibold text-(--interactive)">{actionLabel} <ArrowRight aria-hidden="true" className="transition-transform group-hover:translate-x-1" size={14} /></span>
      </Card>
    </Link>
  );
}
