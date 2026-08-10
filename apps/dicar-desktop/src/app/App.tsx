import { BrowserRouter, Link, Route, Routes } from "react-router";

const destinations = [
  { to: "/live", title: "实时调参与波形", description: "实时参数编辑、写入与波形监测", state: "主工作台" },
  { to: "/records", title: "数据记录与回放", description: "记录设备会话并回放遥测数据", state: "开发中" },
  { to: "/presets", title: "参数方案库", description: "管理、比较和应用参数方案", state: "开发中" },
  { to: "/diagnostics", title: "连接与链路诊断", description: "检查设备连接与通信链路", state: "开发中" },
];

function Home() {
  return (
    <main id="main-content" className="mx-auto min-h-screen max-w-6xl px-4 py-6 sm:px-6">
      <header className="mb-4 flex flex-wrap items-center justify-between gap-3 border-b border-(--border) pb-4">
        <div>
          <p className="m-0 text-sm font-semibold tracking-wide text-(--text)">DiCar Tune</p>
          <p className="mt-1 mb-0 text-xs text-(--text-muted)">车辆控制器调参与遥测工作台</p>
        </div>
        <span className="rounded-[var(--radius)] border border-(--border) bg-(--surface) px-3 py-2 font-mono text-xs tabular-nums text-(--text-muted)">
          项目：未加载
        </span>
      </header>

      <section aria-label="连接状态" className="mb-6 flex items-center justify-between gap-3 rounded-[var(--radius)] border border-(--border) bg-(--surface) px-4 py-3">
        <div>
          <p className="m-0 text-sm font-medium">设备连接</p>
          <p className="mt-1 mb-0 text-xs text-(--text-muted)">请选择工作台以建立串口链路。</p>
        </div>
        <output aria-live="polite" className="font-mono text-sm font-semibold tabular-nums text-(--warning)">
          未连接
        </output>
      </section>

      <section aria-labelledby="destinations-heading">
        <div className="mb-3 flex items-baseline justify-between gap-3">
          <h1 id="destinations-heading" className="m-0 text-lg font-semibold">工作区</h1>
          <span className="font-mono text-xs tabular-nums text-(--text-muted)">4 个目的地</span>
        </div>
        <nav aria-label="应用目的地" className="grid gap-3 md:grid-cols-2">
          {destinations.map((destination) => (
            <Link
              className="rounded-[var(--radius)] border border-(--border) bg-(--surface-raised) p-4 no-underline transition-colors duration-150 ease-out hover:border-(--interactive) hover:bg-(--surface-hover)"
              key={destination.to}
              to={destination.to}
            >
              <div className="flex items-start justify-between gap-4">
                <div>
                  <h2 className="m-0 text-base font-semibold">{destination.title}</h2>
                  <p className="mb-0 mt-2 text-sm text-(--text-muted)">{destination.description}</p>
                </div>
                <span className="shrink-0 rounded-[var(--radius)] border border-(--border) px-2 py-1 text-xs text-(--text-muted)">
                  {destination.state}
                </span>
              </div>
            </Link>
          ))}
        </nav>
      </section>
    </main>
  );
}

function Destination() {
  return (
    <main id="main-content" className="mx-auto min-h-screen max-w-6xl px-4 py-6 sm:px-6">
      <Link className="text-sm text-(--interactive)" to="/">返回工作区</Link>
    </main>
  );
}

export function App() {
  return (
    <BrowserRouter>
      <a className="skip-link" href="#main-content">跳至主要内容</a>
      <Routes>
        <Route element={<Home />} path="/" />
        <Route element={<Destination />} path="*" />
      </Routes>
    </BrowserRouter>
  );
}
