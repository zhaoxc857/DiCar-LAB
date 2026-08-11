import { Gauge, House, Pulse } from "@phosphor-icons/react";
import { Link, NavLink, Outlet } from "react-router";
import { ConnectionStatusBar } from "./ConnectionStatusBar";
import { VehicleSwitcher } from "./VehicleSwitcher";

export function AppShell() {
  return (
    <div className="min-h-screen bg-[radial-gradient(circle_at_top_right,color-mix(in_srgb,var(--interactive)_8%,transparent),transparent_34%)]">
      <a className="skip-link" href="#main-content">跳至主要内容</a>
      <header className="flex min-h-14 flex-wrap items-center justify-between gap-3 border-b border-(--border) bg-(--background) px-4 py-2 lg:px-6">
        <Link className="flex items-center gap-3 no-underline" to="/">
          <span className="grid size-9 place-items-center rounded-[var(--radius)] border border-(--interactive) bg-[color-mix(in_srgb,var(--interactive)_12%,transparent)] text-(--interactive)"><Gauge aria-hidden="true" size={22} weight="duotone" /></span>
          <span><strong className="block text-sm tracking-wide">DiCar Tune</strong><span className="block text-[11px] text-(--text-muted)">竞赛车辆调参与遥测平台</span></span>
        </Link>
        <div className="flex flex-wrap items-center gap-3">
          <nav aria-label="主要导航" className="hidden items-center gap-1 md:flex">
            <NavLink className="rounded px-2 py-1.5 text-xs text-(--text-muted) no-underline hover:text-(--text)" to="/"><House className="mr-1 inline" size={15} />工作区</NavLink>
            <NavLink className="rounded px-2 py-1.5 text-xs text-(--text-muted) no-underline hover:text-(--text)" to="/diagnostics"><Pulse className="mr-1 inline" size={15} />诊断</NavLink>
          </nav>
          <VehicleSwitcher />
        </div>
      </header>
      <ConnectionStatusBar />
      <Outlet />
    </div>
  );
}
