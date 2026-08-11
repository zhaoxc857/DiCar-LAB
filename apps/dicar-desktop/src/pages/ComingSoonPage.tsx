import { ArrowLeft, ClockCountdown } from "@phosphor-icons/react";
import { Link } from "react-router";
import { Card } from "../components/ui/card";

export function ComingSoonPage({ title, scope }: { title: string; scope: string }) {
  return <main className="mx-auto w-full max-w-3xl px-4 py-10" id="main-content"><Link className="inline-flex items-center gap-1 text-xs text-(--interactive)" to="/"><ArrowLeft size={14} />返回工作区</Link><Card className="mt-5 p-8 text-center"><ClockCountdown aria-hidden="true" className="mx-auto text-(--warning)" size={38} weight="duotone" /><h1 className="mb-0 mt-4 text-xl">{title}</h1><p className="mx-auto mb-0 mt-3 max-w-xl text-sm leading-6 text-(--text-muted)">{scope}，首版后续阶段开放。当前页面不会展示虚构数据。</p></Card></main>;
}
