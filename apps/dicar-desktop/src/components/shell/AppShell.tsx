import type { Icon } from "@phosphor-icons/react";
import {
  Database,
  Gauge,
  GearSix,
  House,
  List,
  Pulse,
  Question,
  SlidersHorizontal,
} from "@phosphor-icons/react";
import { useState } from "react";
import { Link, NavLink, Outlet } from "react-router";
import { Button } from "../ui/button";
import { Drawer } from "../ui/drawer";
import { ConnectionDrawer, type ConnectionDrawerSection } from "./ConnectionDrawer";
import { ConnectionStatusChip } from "./ConnectionStatusChip";

const destinations: Array<{ icon: Icon; label: string; to: string; end?: boolean }> = [
  { icon: House, label: "概览", to: "/", end: true },
  { icon: SlidersHorizontal, label: "实时调试", to: "/live/car-01" },
  { icon: Database, label: "波形记录", to: "/records" },
  { icon: Pulse, label: "诊断", to: "/diagnostics" },
];

export function AppShell() {
  const [connectionDrawerOpen, setConnectionDrawerOpen] = useState(false);
  const [connectionDrawerSection, setConnectionDrawerSection] = useState<ConnectionDrawerSection>("connection");
  const [navigationOpen, setNavigationOpen] = useState(false);

  function openConnectionDrawer(section: ConnectionDrawerSection) {
    setConnectionDrawerSection(section);
    setConnectionDrawerOpen(true);
  }

  return (
    <div className="min-h-screen bg-[radial-gradient(circle_at_top_right,color-mix(in_srgb,var(--interactive)_8%,transparent),transparent_34%)]">
      <a className="skip-link" href="#main-content">跳至主要内容</a>
      <header className="sticky top-0 z-30 border-b border-(--border) bg-[color-mix(in_srgb,var(--background)_94%,transparent)] backdrop-blur">
        <div className="flex min-h-16 items-center gap-2 px-3 lg:px-5">
          <Button
            aria-label="打开主导航"
            className="size-11 shrink-0 p-0 lg:hidden"
            onClick={() => setNavigationOpen(true)}
            variant="secondary"
          >
            <List aria-hidden="true" size={20} />
          </Button>
          <Link className="mr-auto flex min-w-0 items-center gap-2 no-underline lg:mr-3" to="/">
            <span className="grid size-10 shrink-0 place-items-center rounded-[var(--radius)] border border-(--interactive) bg-[color-mix(in_srgb,var(--interactive)_10%,transparent)] text-(--interactive)">
              <Gauge aria-hidden="true" size={22} weight="duotone" />
            </span>
            <span className="hidden min-w-0 sm:block">
              <strong className="block text-sm tracking-wide">DiCar Tune</strong>
              <span className="block truncate text-[10px] text-(--text-muted)">精准车辆调参与遥测控制台</span>
            </span>
          </Link>
          <nav aria-label="主要导航" className="hidden min-w-0 flex-1 items-center justify-center gap-1 lg:flex">
            <NavigationLinks />
          </nav>
          <div className="flex shrink-0 items-center gap-2">
            <ConnectionStatusChip onOpen={() => openConnectionDrawer("connection")} />
            <Button
              aria-label="打开硬件帮助"
              className="hidden size-11 p-0 sm:inline-flex"
              onClick={() => openConnectionDrawer("guide")}
              variant="secondary"
            >
              <Question aria-hidden="true" size={19} />
            </Button>
            <Button
              aria-label="打开设置"
              className="hidden size-11 p-0 sm:inline-flex"
              onClick={() => openConnectionDrawer("preferences")}
              variant="secondary"
            >
              <GearSix aria-hidden="true" size={19} />
            </Button>
          </div>
        </div>
      </header>
      <Outlet />

      <Drawer
        description="在窄窗口中切换主要工作区域"
        onOpenChange={setNavigationOpen}
        open={navigationOpen}
        side="left"
        title="主导航"
      >
        <nav aria-label="窄屏主要导航" className="grid gap-2">
          <NavigationLinks onNavigate={() => setNavigationOpen(false)} />
        </nav>
        <div className="mt-5 grid grid-cols-2 gap-2 border-t border-(--border) pt-4">
          <Button onClick={() => { setNavigationOpen(false); openConnectionDrawer("guide"); }} variant="secondary">
            <Question aria-hidden="true" size={17} />硬件帮助
          </Button>
          <Button onClick={() => { setNavigationOpen(false); openConnectionDrawer("preferences"); }} variant="secondary">
            <GearSix aria-hidden="true" size={17} />设置
          </Button>
        </div>
      </Drawer>
      <ConnectionDrawer
        initialSection={connectionDrawerSection}
        onOpenChange={setConnectionDrawerOpen}
        open={connectionDrawerOpen}
      />
    </div>
  );
}

function NavigationLinks({ onNavigate }: { onNavigate?: () => void }) {
  return destinations.map(({ end, icon: DestinationIcon, label, to }) => (
    <NavLink
      className={({ isActive }) => [
        "flex min-h-11 items-center gap-2 rounded-[var(--radius)] border px-3 text-sm font-medium no-underline transition-colors",
        isActive
          ? "border-(--interactive) bg-[color-mix(in_srgb,var(--interactive)_10%,transparent)] text-(--interactive)"
          : "border-transparent text-(--text-muted) hover:border-(--border) hover:bg-(--surface-hover) hover:text-(--text)",
      ].join(" ")}
      end={end}
      key={to}
      onClick={onNavigate}
      to={to}
    >
      <DestinationIcon aria-hidden="true" size={17} />
      {label}
    </NavLink>
  ));
}
