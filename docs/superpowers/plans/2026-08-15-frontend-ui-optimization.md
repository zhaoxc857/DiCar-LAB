# DiCar Tune Frontend UI Optimization Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Main agent only, sequential execution; do not dispatch subagents.

**Goal:** Replace the stale, crowded desktop shell with the approved Precision Console + Pit Wall UI, a state-preserving Standard/Track workbench, a real recordings page, and a frontend-only wireless-flashing integration seam.

**Architecture:** Keep every existing Provider, Zustand domain store, DesktopBridge command, recording controller, and Tauri/Rust boundary unchanged. Recompose those existing capabilities through focused React components: an accessible Radix drawer for connection controls, route-backed pages for real destinations, and CSS-only workbench density modes backed by one non-sensitive persisted UI preference.

**Tech Stack:** React 19, TypeScript 5.9, React Router 7, Zustand 5, Tailwind CSS 4, Radix Dialog, Phosphor Icons, Vitest, Testing Library, Playwright, axe-core.

## Global Constraints

- Modify frontend UI files only. Do not modify Rust/Tauri, DCTP, Bridge command signatures, device state machines, recording rules, playback rules, AI behavior, parameter-write behavior, or IndexedDB schema.
- Standard and Track modes share the same React state, parameter drafts, selected task, telemetry buffer, recording state, playback context, and Bridge instance.
- A mode switch or page reflow must issue zero DesktopBridge commands.
- Keep API keys out of frontend persistence; settings migration must continue removing `aiApiKey` and `aiBaseUrl`.
- Do not implement parameter-scheme import.
- Wireless flashing receives only a typed frontend entry seam and truthful unavailable UI in this plan; do not add Tauri commands or protocol behavior.
- Use Chinese-first UI copy; retain English only for protocol names, engineering abbreviations, units, and data identifiers.
- Verify 1280×720, 1366×768, 1920×1080, a 640 px narrow viewport, keyboard focus, reduced motion, and dark color scheme.
- Delete a file only after `rg` proves it has no remaining imports or route references.
- Each task follows red-green-refactor and ends with an independent commit.

## File Structure Map

### New files

- `apps/dicar-desktop/src/components/ui/drawer.tsx` — shared accessible left/right drawer primitive built on Radix Dialog.
- `apps/dicar-desktop/src/components/shell/ConnectionStatusChip.tsx` — compact always-visible device health trigger.
- `apps/dicar-desktop/src/components/shell/ConnectionDrawer.tsx` — existing connection controls reorganized into connection, guide, and preferences sections.
- `apps/dicar-desktop/src/components/shell/ConnectionDrawer.test.tsx` — connection behavior, focus, hardware guidance, and no-regression tests.
- `apps/dicar-desktop/src/components/shell/FirmwareFlashEntry.tsx` — typed wireless-flash UI seam with truthful unavailable state.
- `apps/dicar-desktop/src/components/shell/FirmwareFlashEntry.test.tsx` — state-label, disabled-action, and callback tests.
- `apps/dicar-desktop/src/components/workbench/RecordingLibrary.tsx` — reusable recording list/import/export/delete surface without modal chrome.
- `apps/dicar-desktop/src/pages/RecordingsPage.tsx` — route-backed recording library and playback owner.
- `apps/dicar-desktop/src/pages/RecordingsPage.test.tsx` — real route, existing repository reuse, replay, and zero-device-command coverage.
- `apps/dicar-desktop/src/test/seededRecordingController.ts` — shared two-record IndexedDB fixture for recording UI tests.
- `apps/dicar-desktop/src/components/home/RecentRecordingsCard.tsx` — newest three completed recordings on Overview.
- `apps/dicar-desktop/src/components/workbench/WorkbenchModeSwitch.tsx` — persisted Standard/Track selector.
- `apps/dicar-desktop/src/components/workbench/WorkbenchLayout.tsx` — one DOM tree with mode-specific CSS grid classes.
- `apps/dicar-desktop/src/components/workbench/WorkbenchContextActions.tsx` — AI, parameter-snapshot, and recordings actions using shared buttons.
- `apps/dicar-desktop/src/components/workbench/TelemetryStrip.tsx` — safe display of existing target/feedback/error/subscription/drop/latency data.
- `apps/dicar-desktop/src/components/workbench/TelemetryStrip.test.tsx` — existing-data and unavailable-state coverage.
- `apps/dicar-desktop/src/components/workbench/ChangeBar.test.tsx` — hidden-zero-dirty and existing action behavior coverage.

### Modified files

- `apps/dicar-desktop/src/app/styles/tokens.css` and `global.css` — semantic surface/radius/density tokens, shell layout, and reduced-motion behavior.
- `apps/dicar-desktop/src/stores/settingsStore.ts` and `settingsStore.test.ts` — settings v4 with `workbenchMode` while preserving secret scrubbing.
- `apps/dicar-desktop/src/components/shell/AppShell.tsx` — four real destinations, narrow navigation, help/settings triggers, status chip, and drawer ownership.
- `apps/dicar-desktop/src/components/shell/HardwareConnectionGuide.tsx` — drawer-safe presentation with unchanged safety copy.
- `apps/dicar-desktop/src/components/shell/VehicleSwitcher.tsx` — shared UI controls and drawer-friendly layout.
- `apps/dicar-desktop/src/app/routes.tsx` — real `/records` route and removal of `/parameter-sets`.
- `apps/dicar-desktop/src/pages/HomePage.tsx`, `HomePage.test.tsx`, `components/home/MenuCard.tsx`, and `ProjectSummary.tsx` — real Overview content and no stale cards/hard-coded identity.
- `apps/dicar-desktop/src/pages/LiveWorkbenchPage.tsx` and `LiveWorkbenchPage.test.tsx` — dual-mode composition, contextual tools, real records navigation, telemetry strip, and unchanged task logic.
- `apps/dicar-desktop/src/components/workbench/RecordingManagerDialog.tsx` and its test — reuse `RecordingLibrary` while preserving modal compatibility.
- `apps/dicar-desktop/src/components/workbench/ChangeBar.tsx` — render only for a non-empty dirty set.
- `apps/dicar-desktop/src/pages/DiagnosticsPage.tsx` and `DiagnosticsPage.test.tsx` — conclusion-first grouping of existing snapshot fields.
- `apps/dicar-desktop/src/app/App.test.tsx` and `e2e/initial-release.spec.ts` — updated navigation, drawer, dual-mode, records, accessibility, and narrow-layout acceptance.
- `README.md`, `docs/user-guide.md`, `docs/development.md`, and `HANDOFF.md` — new navigation and frontend-only architecture.

### Deleted after reference checks

- `apps/dicar-desktop/src/components/shell/ConnectionStatusBar.tsx`
- `apps/dicar-desktop/src/components/shell/ConnectionStatusBar.test.tsx`
- `apps/dicar-desktop/src/pages/ComingSoonPage.tsx`

---

### Task 1: Semantic tokens and settings v4 workbench preference

**Files:**
- Modify: `apps/dicar-desktop/src/app/styles/tokens.css:1-15`
- Modify: `apps/dicar-desktop/src/app/styles/global.css:1-48`
- Modify: `apps/dicar-desktop/src/stores/settingsStore.ts:1-76`
- Test: `apps/dicar-desktop/src/stores/settingsStore.test.ts:1-61`

**Interfaces:**
- Produces: `export type WorkbenchMode = "standard" | "track"`.
- Produces: `workbenchMode: WorkbenchMode` and `saveWorkbenchMode(mode: WorkbenchMode): void` on `useSettingsStore`.
- Preserves: serial settings, `aiModel`, and synchronous legacy secret scrubbing.

- [ ] **Step 1: Write failing migration and persistence tests**

```ts
import {
  migrateSettingsV4,
  scrubLegacyAiSettings,
  useSettingsStore,
} from "./settingsStore";

it("adds the standard workbench mode while migrating settings v3 to v4", () => {
  expect(migrateSettingsV4({
    serialHardwareProfile: "nanoUartWl",
    serialPortName: "COM7",
    serialBaudRate: 115_200,
    aiModel: "deepseek-chat",
  })).toMatchObject({
    serialPortName: "COM7",
    aiModel: "deepseek-chat",
    workbenchMode: "standard",
  });
});

it("accepts only known workbench modes", () => {
  expect(migrateSettingsV4({ workbenchMode: "track" }).workbenchMode).toBe("track");
  expect(migrateSettingsV4({ workbenchMode: "invalid" }).workbenchMode).toBe("standard");
});

it("persists a track-mode preference without persisting AI secrets", () => {
  useSettingsStore.getState().saveWorkbenchMode("track");
  const raw = localStorage.getItem("dicar-tune-settings") ?? "";
  expect(raw).toContain('"workbenchMode":"track"');
  expect(raw).not.toContain("aiApiKey");
  expect(raw).not.toContain("aiBaseUrl");
});
```

- [ ] **Step 2: Run the focused test and confirm the red state**

Run:

```powershell
pnpm --filter @dicar/desktop exec vitest run src/stores/settingsStore.test.ts
```

Expected: FAIL because `migrateSettingsV4`, `workbenchMode`, and `saveWorkbenchMode` do not exist.

- [ ] **Step 3: Implement settings v4 without weakening the security migration**

```ts
export type WorkbenchMode = "standard" | "track";

type PersistedSettingsV4 = {
  serialHardwareProfile: SerialHardwareProfile;
  serialPortName: string;
  serialBaudRate: number;
  aiModel: string;
  workbenchMode: WorkbenchMode;
};

function workbenchMode(value: unknown): WorkbenchMode {
  return value === "track" ? "track" : "standard";
}

export function migrateSettingsV4(persisted: unknown): PersistedSettingsV4 {
  const legacy = (typeof persisted === "object" && persisted !== null
    ? persisted
    : {}) as Record<string, unknown>;
  return {
    serialHardwareProfile: typeof legacy.serialHardwareProfile === "string"
      ? legacy.serialHardwareProfile as SerialHardwareProfile
      : "nanoUartWl",
    serialPortName: typeof legacy.serialPortName === "string" ? legacy.serialPortName : "",
    serialBaudRate: typeof legacy.serialBaudRate === "number"
      && Number.isFinite(legacy.serialBaudRate)
      ? legacy.serialBaudRate
      : 460_800,
    aiModel: typeof legacy.aiModel === "string" && legacy.aiModel.trim().length > 0
      ? legacy.aiModel.trim()
      : DEFAULT_AI_MODEL,
    workbenchMode: workbenchMode(legacy.workbenchMode),
  };
}
```

Update `scrubLegacyAiSettings`, Zustand `version`, `migrate`, `partialize`, and `merge` to v4. Keep the raw-storage rewrite before store creation so plaintext legacy keys are removed before hydration.

- [ ] **Step 4: Add the approved visual tokens**

```css
:root {
  --background: #060d12;
  --surface: #0a151c;
  --surface-raised: #0e1b23;
  --surface-hover: #132630;
  --border: #263a46;
  --border-subtle: color-mix(in srgb, var(--border) 62%, transparent);
  --text: #e8f2f5;
  --text-muted: #91a6ae;
  --interactive: #36d5e4;
  --interactive-strong: #16b8ca;
  --success: #6ae3b2;
  --warning: #f3b95f;
  --danger: #ff7580;
  --focus-ring: #67e8f9;
  --radius-sm: 6px;
  --radius: 8px;
  --radius-lg: 12px;
  --control-height: 40px;
  --control-height-compact: 32px;
}
```

Keep the existing fonts and reduced-motion block. Add reusable `.data-value` and `.app-surface` classes only; do not add page-specific selectors to `global.css`.

- [ ] **Step 5: Run focused tests, lint, and typecheck**

```powershell
pnpm --filter @dicar/desktop exec vitest run src/stores/settingsStore.test.ts
pnpm lint
pnpm typecheck
```

Expected: all commands exit 0 and raw localStorage tests still prove no plaintext API key survives.

- [ ] **Step 6: Commit the preference and token foundation**

```powershell
git add apps/dicar-desktop/src/app/styles/tokens.css apps/dicar-desktop/src/app/styles/global.css apps/dicar-desktop/src/stores/settingsStore.ts apps/dicar-desktop/src/stores/settingsStore.test.ts
git commit -m "feat(app): add UI density preference and semantic tokens"
```

---

### Task 2: Accessible global shell, connection drawer, and firmware seam

**Files:**
- Create: `apps/dicar-desktop/src/components/ui/drawer.tsx`
- Create: `apps/dicar-desktop/src/components/shell/ConnectionStatusChip.tsx`
- Create: `apps/dicar-desktop/src/components/shell/ConnectionDrawer.tsx`
- Create: `apps/dicar-desktop/src/components/shell/ConnectionDrawer.test.tsx`
- Create: `apps/dicar-desktop/src/components/shell/FirmwareFlashEntry.tsx`
- Create: `apps/dicar-desktop/src/components/shell/FirmwareFlashEntry.test.tsx`
- Modify: `apps/dicar-desktop/src/components/shell/AppShell.tsx:1-31`
- Modify: `apps/dicar-desktop/src/components/shell/HardwareConnectionGuide.tsx:1-17`
- Modify: `apps/dicar-desktop/src/components/shell/VehicleSwitcher.tsx:1-38`
- Modify: `apps/dicar-desktop/src/app/App.test.tsx:1-22`
- Delete after migration: `apps/dicar-desktop/src/components/shell/ConnectionStatusBar.tsx` and `ConnectionStatusBar.test.tsx`

**Interfaces:**
- Produces: `Drawer` with `open`, `onOpenChange`, `title`, `description`, and `side` props.
- Produces: `ConnectionDrawerSection = "connection" | "guide" | "preferences"`.
- Produces: `FirmwareFlashUiState` discriminated union and `FirmwareFlashEntryProps`.
- Consumes without change: `DesktopBridge`, `connectSerialWithProbe`, `useConnectionStore`, and `useSettingsStore`.

- [ ] **Step 1: Write failing shell, drawer, and firmware tests**

```tsx
it("opens all connection controls from the compact status chip", async () => {
  render(<AppProviders bridge={new HardwareBridge()}><App /></AppProviders>);
  await act(async () => undefined);
  fireEvent.click(screen.getByRole("button", { name: /未连接.*打开设备连接/ }));
  expect(screen.getByRole("dialog", { name: "设备连接" })).toBeInTheDocument();
  fireEvent.change(screen.getByRole("combobox", { name: "连接方式" }), {
    target: { value: "serial" },
  });
  expect(await screen.findByRole("option", { name: /COM12.*Bluetooth/ })).toBeInTheDocument();
});

it("shows a truthful disabled wireless-flash seam", () => {
  const onOpenFirmwareFlash = vi.fn();
  render(
    <FirmwareFlashEntry
      firmwareVersion={[0, 2, 0]}
      onOpenFirmwareFlash={onOpenFirmwareFlash}
      state={{ kind: "unavailable" }}
    />,
  );
  expect(screen.getByText("固件 0.2.0")).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "无线烧录尚未启用" }));
  expect(onOpenFirmwareFlash).not.toHaveBeenCalled();
});

it.each([
  [{ kind: "checking" }, "正在检查设备"],
  [{ kind: "selecting" }, "选择固件文件"],
  [{ kind: "preparing" }, "正在准备烧录"],
  [{ kind: "flashing", progressPercent: 42 }, "烧录中 42%"],
  [{ kind: "succeeded" }, "烧录成功"],
  [{ kind: "failed", message: "连接中断" }, "烧录失败：连接中断"],
] as const)("labels the reserved workflow state %j", (state, label) => {
  render(<FirmwareFlashEntry firmwareVersion={null} state={state} />);
  expect(screen.getByText(label)).toBeInTheDocument();
});
```

- [ ] **Step 2: Run the focused tests and confirm missing components**

```powershell
pnpm --filter @dicar/desktop exec vitest run src/components/shell/ConnectionDrawer.test.tsx src/components/shell/FirmwareFlashEntry.test.tsx src/app/App.test.tsx
```

Expected: FAIL because the drawer, status chip, firmware entry, and new shell roles are absent.

- [ ] **Step 3: Implement the reusable Radix drawer**

```tsx
import * as Dialog from "@radix-ui/react-dialog";
import { X } from "@phosphor-icons/react";
import type { PropsWithChildren } from "react";
import { cn } from "../../lib/cn";

type DrawerProps = PropsWithChildren<{
  open: boolean;
  onOpenChange: (open: boolean) => void;
  title: string;
  description: string;
  side?: "left" | "right";
}>;

export function Drawer({
  open,
  onOpenChange,
  title,
  description,
  side = "right",
  children,
}: DrawerProps) {
  return (
    <Dialog.Root onOpenChange={onOpenChange} open={open}>
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 z-40 bg-black/65" />
        <Dialog.Content
          className={cn(
            "fixed inset-y-0 z-50 flex w-[min(92vw,440px)] flex-col border-(--border) bg-(--surface-raised) shadow-2xl",
            side === "right" ? "right-0 border-l" : "left-0 border-r",
          )}
        >
          <header className="flex items-start gap-3 border-b border-(--border) p-4">
            <div className="min-w-0 flex-1">
              <Dialog.Title className="m-0 text-base">{title}</Dialog.Title>
              <Dialog.Description className="m-0 mt-1 text-xs text-(--text-muted)">
                {description}
              </Dialog.Description>
            </div>
            <Dialog.Close aria-label={"关闭" + title} className="rounded-[var(--radius-sm)] p-2">
              <X aria-hidden="true" size={18} />
            </Dialog.Close>
          </header>
          <div className="min-h-0 flex-1 overflow-y-auto p-4">{children}</div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
```

- [ ] **Step 4: Extract the existing connection behavior into `ConnectionDrawer`**

Move the local state and the bodies of `selectMode`, `toggleConnection`, `authorizeBrowserPort`, `changeHardwareProfile`, and `errorMessage` from `ConnectionStatusBar.tsx` without changing Bridge calls or result handling.

```tsx
export type ConnectionDrawerSection = "connection" | "guide" | "preferences";

type ConnectionDrawerProps = {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  initialSection: ConnectionDrawerSection;
};

export function ConnectionDrawer({
  open,
  onOpenChange,
  initialSection,
}: ConnectionDrawerProps) {
  const [section, setSection] = useState(initialSection);
  useEffect(() => {
    if (open) setSection(initialSection);
  }, [initialSection, open]);

  return (
    <Drawer
      description="连接设置、硬件安全说明与当前设备信息"
      onOpenChange={onOpenChange}
      open={open}
      title="设备连接"
    >
      <nav aria-label="设备连接分区">
        <Button onClick={() => setSection("connection")} variant="secondary">连接</Button>
        <Button onClick={() => setSection("guide")} variant="secondary">硬件指南</Button>
        <Button onClick={() => setSection("preferences")} variant="secondary">偏好</Button>
      </nav>
      {section === "connection" && connectionControls()}
      {section === "guide" && <HardwareConnectionGuide profile={hardwareProfile} />}
      {section === "preferences" && <VehicleSwitcher />}
      <FirmwareFlashEntry
        firmwareVersion={snapshot?.firmwareVersion ?? null}
        state={{ kind: "unavailable" }}
      />
    </Drawer>
  );
}
```

Define `connectionControls` inside `ConnectionDrawer` so it closes over the migrated state and handlers:

```tsx
function connectionControls() {
  return (
    <section aria-label="连接设置" className="space-y-3">
      <Select
        aria-label="连接方式"
        disabled={ready || busy}
        onChange={(event) => void selectMode(event.currentTarget.value as ConnectionMode)}
        value={mode}
      >
        <option value="simulator">模拟器体验</option>
        <option value="serial">真实串口</option>
      </Select>
      {mode === "serial" && (
        <>
          <Select
            aria-label="硬件类型"
            disabled={ready || busy}
            onChange={(event) =>
              changeHardwareProfile(event.currentTarget.value as SerialHardwareProfile)
            }
            value={hardwareProfile}
          >
            {(Object.entries(HARDWARE_PROFILES) as Array<
              [SerialHardwareProfile, (typeof HARDWARE_PROFILES)[SerialHardwareProfile]]
            >).map(([value, profile]) =>
              <option key={value} value={value}>{profile.label}</option>
            )}
          </Select>
          <Select
            aria-label="选择串口"
            disabled={ready || busy || serialPorts.length === 0}
            onChange={(event) => setSelectedPort(event.currentTarget.value)}
            value={selectedPort}
          >
            <option value="">选择 COM</option>
            {serialPorts.map((port) =>
              <option key={port.portName} value={port.portName}>
                {port.portName + " · " + port.displayName}
              </option>
            )}
          </Select>
          <Select
            aria-label="串口波特率"
            disabled={ready || busy}
            onChange={(event) =>
              setBaudRate(event.currentTarget.value === "auto"
                ? "auto"
                : Number(event.currentTarget.value))
            }
            value={baudRate}
          >
            <option value="auto">自动探测</option>
            {SUPPORTED_SERIAL_BAUD_RATES.map((rate) =>
              <option key={rate} value={rate}>{rate}</option>
            )}
          </Select>
          {bridge.serialAccessMode === "browser" && (
            <Button
              disabled={ready || busy}
              onClick={() => void authorizeBrowserPort()}
              variant="secondary"
            >
              授权浏览器串口
            </Button>
          )}
        </>
      )}
      <Button
        disabled={busy || (!ready && mode === "serial" && selectedPort === "")}
        onClick={() => void toggleConnection()}
        variant={ready ? "secondary" : "primary"}
      >
        {ready
          ? "断开设备"
          : probingRate !== null
            ? "正在探测 " + String(probingRate)
            : mode === "serial" ? "连接真实设备" : "连接模拟器"}
      </Button>
      {(error ?? eventError) && <Alert>{error ?? eventError}</Alert>}
    </section>
  );
}
```

- [ ] **Step 5: Implement the typed wireless-flash seam**

```tsx
export type FirmwareFlashUiState =
  | { kind: "unavailable" }
  | { kind: "checking" }
  | { kind: "selecting" }
  | { kind: "preparing" }
  | { kind: "flashing"; progressPercent: number }
  | { kind: "succeeded" }
  | { kind: "failed"; message: string };

export type FirmwareFlashEntryProps = {
  firmwareVersion: [number, number, number] | null;
  state: FirmwareFlashUiState;
  onOpenFirmwareFlash?: () => void;
};

export function FirmwareFlashEntry({
  firmwareVersion,
  state,
  onOpenFirmwareFlash,
}: FirmwareFlashEntryProps) {
  const version = firmwareVersion === null ? "固件版本未知" : "固件 " + firmwareVersion.join(".");
  const unavailable = state.kind === "unavailable";
  const stateLabel = {
    unavailable: "无线烧录尚未启用",
    checking: "正在检查设备",
    selecting: "选择固件文件",
    preparing: "正在准备烧录",
    flashing: state.kind === "flashing" ? "烧录中 " + String(state.progressPercent) + "%" : "",
    succeeded: "烧录成功",
    failed: state.kind === "failed" ? "烧录失败：" + state.message : "",
  }[state.kind];
  return (
    <section aria-labelledby="firmware-entry-title">
      <h3 id="firmware-entry-title">固件</h3>
      <p>{version}</p>
      <p aria-live="polite">{stateLabel}</p>
      <Button
        disabled={unavailable}
        onClick={onOpenFirmwareFlash}
        variant="secondary"
      >
        {unavailable ? "无线烧录尚未启用" : "打开无线烧录"}
      </Button>
    </section>
  );
}
```

Do not create a `FirmwarePlatform`, Tauri invoke, DesktopBridge method, file picker, or fake progress timer in this task.

- [ ] **Step 6: Recompose `AppShell` around the chip and drawer**

Render four links: 概览, 实时调试, 波形记录, 诊断. Add an accessible narrow-width navigation drawer using the same `Drawer` primitive. The help and settings icon buttons open `ConnectionDrawer` with `guide` and `preferences` respectively. `ConnectionStatusChip` opens it with `connection`.

```tsx
const [drawerOpen, setDrawerOpen] = useState(false);
const [drawerSection, setDrawerSection] = useState<ConnectionDrawerSection>("connection");

function openDrawer(section: ConnectionDrawerSection) {
  setDrawerSection(section);
  setDrawerOpen(true);
}

<ConnectionStatusChip onOpen={() => openDrawer("connection")} />
<Button aria-label="打开硬件帮助" onClick={() => openDrawer("guide")} variant="secondary" />
<Button aria-label="打开设置" onClick={() => openDrawer("preferences")} variant="secondary" />
<ConnectionDrawer
  initialSection={drawerSection}
  onOpenChange={setDrawerOpen}
  open={drawerOpen}
/>
```

- [ ] **Step 7: Run focused and full component tests**

```powershell
pnpm --filter @dicar/desktop exec vitest run src/components/shell/ConnectionDrawer.test.tsx src/components/shell/FirmwareFlashEntry.test.tsx src/app/App.test.tsx src/pages/HomePage.test.tsx src/pages/DiagnosticsPage.test.tsx
pnpm lint
pnpm typecheck
```

Expected: existing simulator, Web Serial, HC-05 guidance, and connection labels still pass through the drawer.

- [ ] **Step 8: Prove the old status bar is unreferenced, delete it, and commit**

```powershell
rg -n "ConnectionStatusBar" apps/dicar-desktop/src
```

Expected before deletion: only the old component and test. Delete both files, rerun the focused tests, then:

```powershell
git add apps/dicar-desktop/src/components/ui/drawer.tsx apps/dicar-desktop/src/components/shell apps/dicar-desktop/src/app/App.test.tsx
git commit -m "feat(app): replace connection bar with device drawer"
```

---

### Task 3: Real recordings page with shared existing behavior

**Files:**
- Create: `apps/dicar-desktop/src/components/workbench/RecordingLibrary.tsx`
- Create: `apps/dicar-desktop/src/pages/RecordingsPage.tsx`
- Create: `apps/dicar-desktop/src/pages/RecordingsPage.test.tsx`
- Create: `apps/dicar-desktop/src/test/seededRecordingController.ts`
- Modify: `apps/dicar-desktop/src/components/workbench/RecordingManagerDialog.tsx:1-111`
- Modify: `apps/dicar-desktop/src/components/workbench/RecordingManagerDialog.test.tsx:1-115`
- Modify: `apps/dicar-desktop/src/app/routes.tsx:1-11`

**Interfaces:**
- Produces: `RecordingLibraryProps = { onReplay(recordingId: string): void; download?: RecordingDownload }`.
- Produces: `seededRecordingController(): Promise<{ bridge: MockBridge; controller: RecordingController }>` for UI tests only.
- Consumes unchanged: `RecordingController.listRecordings`, `deleteRecording`, `getDocument`, `importJson`, and `protect`.
- Preserves: JSON/CSV formats, validation, protected export/playback, and IndexedDB limits.

- [ ] **Step 1: Write a failing route-backed recording library test**

```tsx
it("serves the completed recording library at /records", async () => {
  window.history.pushState({}, "", "/records");
  const { bridge, controller } = await seededRecordingController();
  render(
    <AppProviders bridge={bridge} recordingController={controller}>
      <App />
    </AppProviders>,
  );

  expect(await screen.findByRole("heading", { name: "波形记录" })).toBeInTheDocument();
  expect(screen.getByText("最新记录")).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "回放 最新记录" }));
  expect(screen.getByRole("dialog", { name: "回放 · 最新记录" })).toBeInTheDocument();
});
```

Create the shared test fixture with the same real repository/controller setup currently duplicated in `RecordingManagerDialog.test.tsx`:

```ts
export async function seededRecordingController() {
  const ids = [
    "5fd2817e-0bb8-4510-9478-2ec7f78c84a1",
    "e5d3d9f6-6450-4d5e-9ec3-f18c20c24d89",
  ];
  let idIndex = 0;
  let now = 1_000;
  const repository = new RecordingRepository({
    indexedDb: new IDBFactory(),
    databaseName: "recording-ui-" + crypto.randomUUID(),
  });
  const controller = new RecordingController(repository, {
    idFactory: () => ids[idIndex++] as string,
    now: () => now,
  });
  const bridge = new MockBridge();
  await bridge.connect({ kind: "simulator", address: "127.0.0.1:7100" });
  controller.setSnapshot(await bridge.getSnapshot());
  await controller.start({ name: "较早记录", note: "first", vehicleProfileId: "generic-manifest" });
  await controller.stop("manual");
  now = 2_000;
  await controller.start({ name: "最新记录", note: "second", vehicleProfileId: "generic-manifest" });
  await controller.stop("manual");
  return { bridge, controller };
}
```

- [ ] **Step 2: Run the page test and confirm `/records` still renders ComingSoon**

```powershell
pnpm --filter @dicar/desktop exec vitest run src/pages/RecordingsPage.test.tsx
```

Expected: FAIL because `RecordingsPage` does not exist and the route is still deferred.

- [ ] **Step 3: Extract modal-independent `RecordingLibrary`**

Move the existing `recordings`, `message`, `busyId` state and the `refresh`, `remove`, `exportRecording`, and `importFile` functions from `RecordingManagerDialog` unchanged.
Move `RecordingDownload` and `downloadBlob` into `RecordingLibrary.tsx`; the dialog imports and re-exports the type only if an existing caller requires it.

```tsx
export type RecordingLibraryProps = {
  onReplay: (recordingId: string) => void;
  download?: RecordingDownload;
};

export function RecordingLibrary({
  onReplay,
  download = downloadBlob,
}: RecordingLibraryProps) {
  const controller = useRecordingController();
  const [recordings, setRecordings] = useState<TelemetryRecordingMetadata[]>([]);
  const [message, setMessage] = useState<string | null>(null);
  const [busyId, setBusyId] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    void controller.listRecordings()
      .then((items) => {
        if (!cancelled) setRecordings(items);
      })
      .catch(() => {
        if (!cancelled) setMessage("无法读取波形记录库");
      });
    return () => {
      cancelled = true;
    };
  }, [controller]);

  return (
    <section aria-label="波形记录库内容" className="space-y-4">
      <label>
        导入 JSON
        <input
          accept="application/json,.json"
          aria-label="导入记录 JSON"
          disabled={busyId !== null}
          onChange={(event) => {
            const file = event.currentTarget.files?.[0];
            event.currentTarget.value = "";
            if (file) void importFile(file);
          }}
          type="file"
        />
      </label>
      {message !== null && <p aria-live="polite">{message}</p>}
      {recordings.length === 0
        ? <p>还没有完整波形记录。</p>
        : (
          <ul>
            {recordings.map((recording) => (
              <li data-testid="recording-row" key={recording.id}>
                <strong>{recording.name}</strong>
                <Button onClick={() => onReplay(recording.id)} variant="secondary">
                  回放
                </Button>
                <Button
                  onClick={() => void exportRecording(recording, "json")}
                  variant="secondary"
                >
                  JSON
                </Button>
                <Button
                  onClick={() => void exportRecording(recording, "csv")}
                  variant="secondary"
                >
                  CSV
                </Button>
                <Button onClick={() => void remove(recording)} variant="danger">
                  删除
                </Button>
              </li>
            ))}
          </ul>
        )}
    </section>
  );
}
```

Retain the current per-recording aria-labels, metadata line, note, disabled-busy behavior, stop-reason labels, and download filename rules when moving the markup.

- [ ] **Step 4: Make the dialog a thin compatibility wrapper**

```tsx
export function RecordingManagerDialog({
  open,
  onClose,
  onReplay,
  download,
}: Props) {
  if (!open) return null;
  return (
    <div className="fixed inset-0 z-50 grid place-items-center bg-black/70 p-4">
      <section aria-labelledby="recording-manager-title" aria-modal="true" role="dialog">
        <header>
          <h2 id="recording-manager-title">波形记录库</h2>
          <Button aria-label="关闭波形记录库" onClick={onClose} variant="secondary">
            关闭
          </Button>
        </header>
        <RecordingLibrary download={download} onReplay={onReplay} />
      </section>
    </div>
  );
}
```

- [ ] **Step 5: Implement `RecordingsPage` and register the route**

```tsx
export function RecordingsPage() {
  const [playbackRecordingId, setPlaybackRecordingId] = useState<string | null>(null);
  return (
    <main className="mx-auto w-full max-w-7xl px-4 py-6 lg:px-6" id="main-content">
      <header>
        <h1>波形记录</h1>
        <p>完整原始批次、独立回放与安全导入导出。</p>
      </header>
      <RecordingLibrary onReplay={setPlaybackRecordingId} />
      <RecordingPlaybackDialog
        onClose={() => setPlaybackRecordingId(null)}
        open={playbackRecordingId !== null}
        recordingId={playbackRecordingId}
      />
    </main>
  );
}
```

Replace the `ComingSoonPage` element at path `records` with `RecordingsPage`. Do not change `RecordingController` or recording-domain files.

- [ ] **Step 6: Run existing recording and page tests**

```powershell
pnpm --filter @dicar/desktop exec vitest run src/pages/RecordingsPage.test.tsx src/components/workbench/RecordingManagerDialog.test.tsx src/components/workbench/RecordingPlaybackDialog.test.tsx src/stores/recordingStore.test.ts src/telemetry/recordings.test.ts src/telemetry/recordingRepository.test.ts
pnpm typecheck
```

Expected: all tests pass with the existing JSON/CSV, protection, playback, and storage behavior.

- [ ] **Step 7: Commit the real records destination**

```powershell
git add apps/dicar-desktop/src/components/workbench/RecordingLibrary.tsx apps/dicar-desktop/src/components/workbench/RecordingManagerDialog.tsx apps/dicar-desktop/src/components/workbench/RecordingManagerDialog.test.tsx apps/dicar-desktop/src/pages/RecordingsPage.tsx apps/dicar-desktop/src/pages/RecordingsPage.test.tsx apps/dicar-desktop/src/test/seededRecordingController.ts apps/dicar-desktop/src/app/routes.tsx
git commit -m "feat(app): promote waveform records to a real page"
```

---

### Task 4: Truthful Overview and stale-route removal

**Files:**
- Create: `apps/dicar-desktop/src/components/home/RecentRecordingsCard.tsx`
- Modify: `apps/dicar-desktop/src/pages/HomePage.tsx:1-32`
- Modify: `apps/dicar-desktop/src/pages/HomePage.test.tsx:1-96`
- Modify: `apps/dicar-desktop/src/components/home/MenuCard.tsx:1-31`
- Modify: `apps/dicar-desktop/src/components/home/ProjectSummary.tsx:1-25`
- Modify: `apps/dicar-desktop/src/app/routes.tsx:1-12`
- Modify: `apps/dicar-desktop/src/app/App.test.tsx:1-23`
- Delete: `apps/dicar-desktop/src/pages/ComingSoonPage.tsx`

**Interfaces:**
- Produces: `RecentRecordingsCard({ limit?: number })` with default limit 3.
- Consumes: `useRecordingController` and `useVehicleProfileStore`.
- Removes: `/parameter-sets` top-level route while preserving the existing in-workbench parameter snapshot manager.

- [ ] **Step 1: Replace stale Home assertions with failing truthful-overview assertions**

```tsx
expect(screen.getByRole("heading", { name: "概览" })).toBeInTheDocument();
expect(screen.getByRole("link", { name: /进入实时调试/ })).toBeInTheDocument();
expect(screen.getByRole("link", { name: /打开波形记录/ })).toBeInTheDocument();
expect(screen.getByRole("link", { name: /查看诊断/ })).toBeInTheDocument();
expect(screen.queryByText("计划发布")).not.toBeInTheDocument();
expect(screen.queryByText("参数方案库")).not.toBeInTheDocument();
expect(screen.queryByText("CAR-01 / DEFAULT")).not.toBeInTheDocument();
expect(screen.getByText("通用 Manifest")).toBeInTheDocument();
```

- [ ] **Step 2: Run Home and App tests and confirm stale-copy failures**

```powershell
pnpm --filter @dicar/desktop exec vitest run src/pages/HomePage.test.tsx src/app/App.test.tsx
```

Expected: FAIL on the old 工作区 title, planned-release cards, and hard-coded project identity.

- [ ] **Step 3: Implement real vehicle/project identity**

```tsx
function selectedVehicleLabel(
  selectedProfileId: string,
  userProfiles: StoredVehicleProfile[],
): string {
  if (selectedProfileId === GENERIC_PROFILE_ID) return "通用 Manifest";
  return [...builtInProfiles, ...userProfiles]
    .find((entry) => entry.profile.vehicle.id === selectedProfileId)
    ?.profile.vehicle.displayName ?? "通用 Manifest";
}
```

Render the selected profile label, `snapshot.deviceIdHex ?? "设备未连接"`, firmware version, parameter count, telemetry count, dirty count, and storage generation. Do not synthesize a vehicle ID.

- [ ] **Step 4: Implement the recent-recordings card**

```tsx
export function RecentRecordingsCard({ limit = 3 }: { limit?: number }) {
  const controller = useRecordingController();
  const [recordings, setRecordings] = useState<TelemetryRecordingMetadata[]>([]);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    let cancelled = false;
    void controller.listRecordings()
      .then((items) => {
        if (!cancelled) setRecordings(items.slice(0, limit));
      })
      .catch(() => {
        if (!cancelled) setError("无法读取最近波形记录");
      });
    return () => {
      cancelled = true;
    };
  }, [controller, limit]);
  return (
    <Card>
      <h2>最近记录</h2>
      {error !== null
        ? <p role="status">{error}</p>
        : recordings.length === 0
        ? <p>还没有完整波形记录。</p>
        : recordings.map((recording) => <p key={recording.id}>{recording.name}</p>)}
      <Link to="/records">查看全部记录</Link>
    </Card>
  );
}
```

- [ ] **Step 5: Rebuild Home around three real actions**

Change the heading to 概览. Render cards for 实时调试, 波形记录, and 诊断 only. Simplify `MenuCardProps` to remove the planned-release status union; every rendered destination is real.

Use `/live` as the real workbench route and remove the hard-coded `car-01` destination. Keep a compatibility redirect only:

```tsx
<Route element={<LiveWorkbenchPage />} path="live" />
<Route element={<Navigate replace to="/live" />} path="live/:vehicleId" />
```

Remove the `parameter-sets` route. Confirm `SnapshotManagerDialog` and the 参数方案 button in LiveWorkbench remain unchanged.

- [ ] **Step 6: Prove ComingSoon is unreferenced and delete it**

```powershell
rg -n "ComingSoonPage|parameter-sets|计划发布" apps/dicar-desktop/src
```

Expected before deletion: no imports or routes; only the obsolete file may remain. Delete `ComingSoonPage.tsx`, then run:

```powershell
pnpm --filter @dicar/desktop exec vitest run src/pages/HomePage.test.tsx src/app/App.test.tsx
pnpm lint
pnpm typecheck
```

- [ ] **Step 7: Commit the truthful Overview**

```powershell
git add apps/dicar-desktop/src/pages/HomePage.tsx apps/dicar-desktop/src/pages/HomePage.test.tsx apps/dicar-desktop/src/components/home apps/dicar-desktop/src/app/routes.tsx apps/dicar-desktop/src/app/App.test.tsx
git commit -m "feat(app): replace stale home cards with truthful overview"
```

---

### Task 5: State-preserving Standard/Track workbench composition

**Files:**
- Create: `apps/dicar-desktop/src/components/workbench/WorkbenchModeSwitch.tsx`
- Create: `apps/dicar-desktop/src/components/workbench/WorkbenchLayout.tsx`
- Create: `apps/dicar-desktop/src/components/workbench/WorkbenchContextActions.tsx`
- Modify: `apps/dicar-desktop/src/pages/LiveWorkbenchPage.tsx:1-124`
- Modify: `apps/dicar-desktop/src/pages/LiveWorkbenchPage.test.tsx:1-110`

**Interfaces:**
- Consumes: `WorkbenchMode`, `workbenchMode`, and `saveWorkbenchMode` from Task 1.
- Produces: `WorkbenchLayoutProps` with three ReactNode slots and one unchanging DOM order.
- Preserves: all existing task-selection, parameter-selection, recommendation, AutoTune, and SnapshotManager state.

- [ ] **Step 1: Add a failing zero-command mode-switch test**

```tsx
it("switches Standard and Track density without issuing device commands", async () => {
  window.history.pushState({}, "", "/live");
  const bridge = new MockBridge();
  const subscription = vi.spyOn(bridge, "setTelemetrySubscription");
  const paused = vi.spyOn(bridge, "setPaused");
  const write = vi.spyOn(bridge, "writeParameter");
  const commit = vi.spyOn(bridge, "commitParameters");
  render(<AppProviders bridge={bridge}><App /></AppProviders>);
  await act(async () => undefined);

  fireEvent.click(screen.getByRole("button", { name: "赛道模式" }));
  expect(screen.getByTestId("workbench-layout")).toHaveAttribute(
    "data-workbench-mode",
    "track",
  );
  expect(subscription).not.toHaveBeenCalled();
  expect(paused).not.toHaveBeenCalled();
  expect(write).not.toHaveBeenCalled();
  expect(commit).not.toHaveBeenCalled();
});
```

- [ ] **Step 2: Run the LiveWorkbench test and confirm the mode controls are absent**

```powershell
pnpm --filter @dicar/desktop exec vitest run src/pages/LiveWorkbenchPage.test.tsx
```

Expected: FAIL because 赛道模式 and `data-workbench-mode` do not exist.

- [ ] **Step 3: Implement the persisted mode switch**

```tsx
export function WorkbenchModeSwitch() {
  const mode = useSettingsStore((state) => state.workbenchMode);
  const save = useSettingsStore((state) => state.saveWorkbenchMode);
  return (
    <div aria-label="工作台模式" role="group">
      <Button
        aria-pressed={mode === "standard"}
        onClick={() => save("standard")}
        variant="secondary"
      >
        标准模式
      </Button>
      <Button
        aria-pressed={mode === "track"}
        onClick={() => save("track")}
        variant="secondary"
      >
        赛道模式
      </Button>
    </div>
  );
}
```

- [ ] **Step 4: Implement one DOM tree with mode-specific layout classes**

```tsx
type WorkbenchLayoutProps = {
  mode: WorkbenchMode;
  navigation: ReactNode;
  editor: ReactNode;
  waveform: ReactNode;
};

export function WorkbenchLayout({
  mode,
  navigation,
  editor,
  waveform,
}: WorkbenchLayoutProps) {
  return (
    <div
      className={cn(
        "mt-3 grid min-h-[560px] gap-3",
        mode === "standard"
          ? "xl:grid-cols-[264px_minmax(420px,1fr)_minmax(440px,1.15fr)]"
          : "xl:grid-cols-[196px_minmax(320px,.78fr)_minmax(520px,1.45fr)]",
      )}
      data-testid="workbench-layout"
      data-workbench-mode={mode}
    >
      <div className={mode === "track" ? "space-y-2" : "space-y-3"}>{navigation}</div>
      <div>{editor}</div>
      <div>{waveform}</div>
    </div>
  );
}
```

Do not conditionally mount different editor or waveform component types. Only class names change.

- [ ] **Step 5: Move header tools into `WorkbenchContextActions`**

```tsx
type WorkbenchContextActionsProps = {
  onOpenAutoTune: () => void;
  onOpenSnapshots: () => void;
  revision: number;
};

export function WorkbenchContextActions({
  onOpenAutoTune,
  onOpenSnapshots,
  revision,
}: WorkbenchContextActionsProps) {
  return (
    <div className="flex flex-wrap items-center gap-2">
      <Button onClick={onOpenAutoTune} size="sm" variant="secondary">AI 调参</Button>
      <Button onClick={onOpenSnapshots} size="sm" variant="secondary">参数方案</Button>
      <Link to="/records">波形记录</Link>
      <span className="data-value">RAM ≠ FLASH · ACK TRUTH · REV {revision}</span>
    </div>
  );
}
```

Remove `recordingsOpen` and `playbackRecordingId` from `LiveWorkbenchPage`. Keep recording start/stop inside `WaveformPanel` and route library access to `/records`.

- [ ] **Step 6: Recompose LiveWorkbench without changing selection logic**

Keep every effect and helper from lines 47–92 unchanged. Replace only the header and the outer three-column div with `WorkbenchModeSwitch`, `WorkbenchContextActions`, and `WorkbenchLayout`. Pass the existing `WorkspaceNav`, `ParameterNav`, `TaskEditor`, and `WaveformPanel` elements as slots.

- [ ] **Step 7: Run workbench, AI, waveform, and snapshot tests**

```powershell
pnpm --filter @dicar/desktop exec vitest run src/pages/LiveWorkbenchPage.test.tsx src/components/workbench/AutoTuneWizard.test.tsx src/components/workbench/WaveformPanel.test.tsx src/components/workbench/SnapshotManagerDialog.test.tsx src/components/workbench/ControlLoopWorkspace.test.tsx
pnpm lint
pnpm typecheck
```

Expected: mode test passes with zero Bridge calls; all existing tuning and recording controls remain operational.

- [ ] **Step 8: Commit the dual-mode workbench**

```powershell
git add apps/dicar-desktop/src/components/workbench/WorkbenchModeSwitch.tsx apps/dicar-desktop/src/components/workbench/WorkbenchLayout.tsx apps/dicar-desktop/src/components/workbench/WorkbenchContextActions.tsx apps/dicar-desktop/src/pages/LiveWorkbenchPage.tsx apps/dicar-desktop/src/pages/LiveWorkbenchPage.test.tsx
git commit -m "feat(app): add state-preserving workbench modes"
```

---

### Task 6: Existing-data telemetry strip and conditional ChangeBar

**Files:**
- Create: `apps/dicar-desktop/src/components/workbench/TelemetryStrip.tsx`
- Create: `apps/dicar-desktop/src/components/workbench/TelemetryStrip.test.tsx`
- Create: `apps/dicar-desktop/src/components/workbench/ChangeBar.test.tsx`
- Modify: `apps/dicar-desktop/src/components/workbench/ChangeBar.tsx:1-24`
- Modify: `apps/dicar-desktop/src/pages/LiveWorkbenchPage.tsx`
- Modify: `apps/dicar-desktop/src/pages/LiveWorkbenchPage.test.tsx`

**Interfaces:**
- Produces: `TelemetryStripItem = { id; label; value; unit; tone }`.
- Consumes: selected `ResolvedControlLoop`, existing `ParameterSnapshot[]`, `TelemetryDescriptor[]`, `TelemetryRingBuffer`, and `AppSnapshot | null`.
- Never estimates values absent from those sources.

- [ ] **Step 1: Write failing tests for real and unavailable values**

```tsx
it("shows target, feedback, error, subscription, drop, and latency from existing data", () => {
  const buffer = new TelemetryRingBuffer(8, 100);
  buffer.append([
    { channelId: 205, sampleSequence: 1, timestampUs: 10, value: { kind: "f32", value: 1.17 } },
    { channelId: 206, sampleSequence: 1, timestampUs: 10, value: { kind: "f32", value: -0.03 } },
  ]);
  render(
    <TelemetryStrip
      buffer={buffer}
      descriptors={descriptors}
      loop={loop}
      records={records}
      snapshot={snapshot}
    />,
  );
  expect(screen.getByText("1.200")).toBeInTheDocument();
  expect(screen.getByText("1.170")).toBeInTheDocument();
  expect(screen.getByText("-0.030")).toBeInTheDocument();
  expect(screen.getByText("500 Hz")).toBeInTheDocument();
  expect(screen.getByText("8.4 ms")).toBeInTheDocument();
});

it("uses an em dash instead of inventing missing telemetry", () => {
  render(
    <TelemetryStrip
      buffer={new TelemetryRingBuffer(8, 100)}
      descriptors={[]}
      loop={undefined}
      records={[]}
      snapshot={null}
    />,
  );
  expect(screen.getAllByText("—").length).toBeGreaterThan(0);
});
```

- [ ] **Step 2: Write a failing test that zero dirty items remove the bar**

```tsx
it("does not occupy the viewport when there are no dirty parameters", () => {
  render(
    <AppProviders bridge={new MockBridge()}>
      <ChangeBar dirtyCount={0} onReview={() => undefined} />
    </AppProviders>,
  );
  expect(screen.queryByText("0 项待固化")).not.toBeInTheDocument();
});
```

- [ ] **Step 3: Run both component tests and confirm red state**

```powershell
pnpm --filter @dicar/desktop exec vitest run src/components/workbench/TelemetryStrip.test.tsx src/components/workbench/ChangeBar.test.tsx
```

Expected: FAIL because `TelemetryStrip` is absent and `ChangeBar` still renders for zero dirty items.

- [ ] **Step 4: Implement safe telemetry item derivation**

```ts
export type TelemetryStripItem = {
  id: "target" | "feedback" | "error" | "subscription" | "drop" | "latency";
  label: string;
  value: string;
  unit: string;
  tone: "default" | "success" | "warning";
};

function channelValue(
  channelId: number | null | undefined,
  descriptors: TelemetryDescriptor[],
  buffer: TelemetryRingBuffer,
): { value: string; unit: string } {
  if (channelId === null || channelId === undefined) return { value: "—", unit: "" };
  const point = buffer.latest(channelId);
  const descriptor = descriptors.find((item) => item.channelId === channelId);
  return point === undefined
    ? { value: "—", unit: descriptor?.unit ?? "" }
    : { value: point.value.value.toFixed(3), unit: descriptor?.unit ?? "" };
}
```

For target, prefer the resolved target telemetry channel; if absent, use the current RAM value of `loop.targetParamId`. For drops, display the sum of `sequenceGapSamples` and `deviceDroppedSamples` with label 丢样. For subscription and latency, use `activeSubscription.sampleRateHz` and `diagnostics.lastRttMs`. Do not label subscription frequency as measured RX rate.

- [ ] **Step 5: Hide `ChangeBar` only when dirty count is zero**

```tsx
export function ChangeBar({ dirtyCount, onReview }: ChangeBarProps) {
  if (dirtyCount === 0) return null;
  return <VisibleChangeBar dirtyCount={dirtyCount} onReview={onReview} />;
}

function VisibleChangeBar({ dirtyCount, onReview }: ChangeBarProps) {
  const bridge = useDesktopBridge();
  const profile = useCollaborationStore((state) => state.profile);
  const [message, setMessage] = useState<string | null>(null);
  const commitReason = profile.role !== "owner"
    ? "当前身份没有固化权限"
    : !profile.leaseActive
      ? "当前车辆控制权未激活"
      : null;
  async function run(action: "undo" | "revert") {
    const result = action === "undo" ? await bridge.undoLast() : await bridge.revertAll();
    setMessage(result.message);
  }
  return (
    <aside>
      <strong>{dirtyCount} 项待固化</strong>
      <p>{message ?? commitReason ?? "RAM 修改已由设备确认，可审阅后固化"}</p>
      <Button onClick={() => void run("undo")} variant="secondary">撤销上次</Button>
      <Button onClick={() => void run("revert")} variant="secondary">全部回退</Button>
      <Button disabled={commitReason !== null} onClick={onReview}>审阅并固化</Button>
    </aside>
  );
}
```

Define `type ChangeBarProps = { dirtyCount: number; onReview: () => void }` and retain the existing sticky classes on the returned aside. Keeping hooks inside `VisibleChangeBar` preserves the Rules of Hooks when dirty count changes between renders.

- [ ] **Step 6: Add the strip to LiveWorkbench and update observer assertions**

Render `TelemetryStrip` between the page header and `LeasePanel`. Pass `selectedLoop` and existing snapshot/records/descriptors/buffer. Update the observer test to expect no change bar before a permitted dirty write instead of expecting a disabled zero-dirty review button.

- [ ] **Step 7: Run focused and full workbench tests**

```powershell
pnpm --filter @dicar/desktop exec vitest run src/components/workbench/TelemetryStrip.test.tsx src/components/workbench/ChangeBar.test.tsx src/pages/LiveWorkbenchPage.test.tsx src/components/workbench/TypedParameterControl.test.tsx
pnpm lint
pnpm typecheck
```

- [ ] **Step 8: Commit telemetry hierarchy and conditional actions**

```powershell
git add apps/dicar-desktop/src/components/workbench/TelemetryStrip.tsx apps/dicar-desktop/src/components/workbench/TelemetryStrip.test.tsx apps/dicar-desktop/src/components/workbench/ChangeBar.tsx apps/dicar-desktop/src/components/workbench/ChangeBar.test.tsx apps/dicar-desktop/src/pages/LiveWorkbenchPage.tsx apps/dicar-desktop/src/pages/LiveWorkbenchPage.test.tsx
git commit -m "feat(app): surface live metrics and hide empty change actions"
```

---

### Task 7: Conclusion-first Diagnostics and accessibility polish

**Files:**
- Modify: `apps/dicar-desktop/src/pages/DiagnosticsPage.tsx:1-37`
- Modify: `apps/dicar-desktop/src/pages/DiagnosticsPage.test.tsx:1-32`
- Modify: `apps/dicar-desktop/src/components/shell/AppShell.tsx`
- Modify: `apps/dicar-desktop/src/app/styles/global.css`

**Interfaces:**
- Consumes only existing `AppSnapshot` fields.
- Produces no new store, Bridge, or diagnostic type.

- [ ] **Step 1: Write failing semantic-section tests**

```tsx
expect(screen.getByRole("heading", { name: "设备健康" })).toBeInTheDocument();
expect(screen.getByRole("heading", { name: "连接质量" })).toBeInTheDocument();
expect(screen.getByRole("heading", { name: "协议事件" })).toBeInTheDocument();
expect(screen.getByText("CRC 错误")).toBeInTheDocument();
expect(screen.getByText("设备丢样")).toBeInTheDocument();
expect(screen.getByText("UI 丢批次")).toBeInTheDocument();
expect(screen.getByText("直接来自设备与 AppActor 快照")).toBeInTheDocument();
```

- [ ] **Step 2: Run Diagnostics tests and confirm the grouping is absent**

```powershell
pnpm --filter @dicar/desktop exec vitest run src/pages/DiagnosticsPage.test.tsx
```

Expected: FAIL because the current page has one flat metric grid.

- [ ] **Step 3: Recompose existing fields into three sections**

```tsx
<section aria-labelledby="device-health">
  <h2 id="device-health">设备健康</h2>
  <Identity label="端点" value={endpoint} />
  <Identity label="会话 ID" value={session} />
  <Identity label="固件版本" value={firmware} />
  <Identity label="设备 ID" value={snapshot?.deviceIdHex ?? "—"} />
</section>
<section aria-labelledby="link-quality">
  <h2 id="link-quality">连接质量</h2>
  <Metric label="往返时延" value={String(diagnostics?.lastRttMs ?? 0) + " ms"} />
  <Metric label="设备丢样" value={diagnostics?.deviceDroppedSamples ?? 0} />
  <Metric label="UI 丢批次" value={diagnostics?.uiDroppedBatches ?? 0} />
</section>
<section aria-labelledby="protocol-events">
  <h2 id="protocol-events">协议事件</h2>
  <details>
    <summary>展开原始协议计数</summary>
    <Metric label="有效帧" value={diagnostics?.validFrames ?? 0} />
    <Metric label="CRC 错误" value={diagnostics?.crcErrors ?? 0} />
    <Metric label="解码溢出" value={diagnostics?.decoderOverflows ?? 0} />
  </details>
</section>
```

Define the local metric component in the same file:

```tsx
function Metric({ label, value }: { label: string; value: string | number }) {
  return (
    <Card className="p-3">
      <span className="block text-[11px] text-(--text-muted)">{label}</span>
      <strong className="data-value mt-1 block">{value}</strong>
    </Card>
  );
}
```

Keep every currently displayed field available. Put raw counters behind `details`; do not calculate health scores or recommendations not already represented by snapshot facts.

- [ ] **Step 4: Add shell focus and narrow-navigation safeguards**

Ensure the collapsed navigation uses a named button, the drawer closes after navigation, every icon-only button has an aria-label, and `#main-content` exists exactly once per route. Keep `.skip-link` and global `:focus-visible` styling.

- [ ] **Step 5: Run component accessibility-focused tests**

```powershell
pnpm --filter @dicar/desktop exec vitest run src/pages/DiagnosticsPage.test.tsx src/app/App.test.tsx src/components/shell/ConnectionDrawer.test.tsx
pnpm lint
pnpm typecheck
```

- [ ] **Step 6: Commit Diagnostics and accessibility structure**

```powershell
git add apps/dicar-desktop/src/pages/DiagnosticsPage.tsx apps/dicar-desktop/src/pages/DiagnosticsPage.test.tsx apps/dicar-desktop/src/components/shell/AppShell.tsx apps/dicar-desktop/src/app/styles/global.css
git commit -m "feat(app): organize diagnostics and accessible shell states"
```

---

### Task 8: Playwright acceptance, visual inspection, and obsolete-file audit

**Files:**
- Modify: `apps/dicar-desktop/e2e/initial-release.spec.ts:1-260`
- Modify as test failures require: frontend UI files from Tasks 2–7 only.

**Interfaces:**
- Uses MockBridge through the existing browser default.
- Verifies the frontend contract without calling DeepSeek or adding backend hooks.

- [ ] **Step 1: Add one connection helper and update existing E2E flows**

```ts
async function openConnectionDrawer(page: Page): Promise<void> {
  await page.getByRole("button", { name: /打开设备连接/ }).click();
  await expect(page.getByRole("dialog", { name: "设备连接" })).toBeVisible();
}

async function connectSimulator(page: Page): Promise<void> {
  await openConnectionDrawer(page);
  await page.getByRole("button", { name: "连接模拟器" }).click();
  await expect(page.getByText("已就绪")).toBeVisible();
  await page.getByRole("button", { name: "关闭设备连接" }).click();
}
```

Replace direct header-bar connection steps with `connectSimulator`. Keep existing tuning, permissions, waveform keyboard, parameter snapshot, recording, replay, and download assertions.

- [ ] **Step 2: Add failing acceptance for dual mode and real records navigation**

```ts
test("标准与赛道模式共享状态且记录库是独立真实页面", async ({ page }) => {
  await page.goto("/live");
  await connectSimulator(page);
  await page.getByLabel("速度环 Kp").fill("1.8");
  await page.getByRole("button", { name: "赛道模式" }).click();
  await expect(page.getByTestId("workbench-layout")).toHaveAttribute(
    "data-workbench-mode",
    "track",
  );
  await expect(page.getByLabel("速度环 Kp")).toHaveValue("1.8");
  await page.getByRole("link", { name: "波形记录" }).click();
  await expect(page.getByRole("heading", { name: "波形记录" })).toBeVisible();
  await expect(page.getByText(/即将推出|计划发布/)).toHaveCount(0);
});
```

- [ ] **Step 3: Add narrow navigation and axe acceptance**

```ts
import AxeBuilder from "@axe-core/playwright";

test("窄窗口导航和关键页面没有严重可访问性问题", async ({ page }) => {
  await page.setViewportSize({ width: 640, height: 720 });
  await page.goto("/");
  await page.getByRole("button", { name: "打开主导航" }).click();
  await expect(page.getByRole("link", { name: "波形记录" })).toBeVisible();
  const results = await new AxeBuilder({ page })
    .withTags(["wcag2a", "wcag2aa"])
    .analyze();
  expect(results.violations.filter((item) =>
    item.impact === "critical" || item.impact === "serious"
  )).toEqual([]);
});
```

- [ ] **Step 4: Run the new E2E tests in red, then fix frontend-only issues**

```powershell
pnpm --filter @dicar/desktop exec playwright test e2e/initial-release.spec.ts
```

Expected first run: FAIL on selectors or responsive details not yet aligned. Fix only React/CSS/accessibility output; do not add Bridge or backend behavior.

- [ ] **Step 5: Capture and inspect four target layouts**

Run the app through Playwright at 1280×720, 1366×768, 1920×1080, and 640×720. Capture Overview, Standard workbench, Track workbench, open connection drawer, Records, and Diagnostics screenshots into the ignored test-results directory. Inspect every image for clipped controls, horizontal overflow, overlapping sticky actions, unreadable units, and missing focus states.

Create one additional 1366×768 context at 150% device scale:

```ts
const highDpiContext = await browser.newContext({
  colorScheme: "dark",
  deviceScaleFactor: 1.5,
  reducedMotion: "reduce",
  viewport: { width: 1366, height: 768 },
});
```

Use these exact acceptance conditions:

```ts
expect(await page.evaluate(() =>
  document.documentElement.scrollWidth <= document.documentElement.clientWidth
)).toBe(true);
```

- [ ] **Step 6: Audit obsolete references**

```powershell
rg -n "ComingSoonPage|ConnectionStatusBar|parameter-sets|计划发布|CAR-01 / DEFAULT" apps/dicar-desktop/src apps/dicar-desktop/e2e
```

Expected: no matches. Also run:

```powershell
rg -n "Operations console|RAM ≠ FLASH|ACK TRUTH" apps/dicar-desktop/src
```

Expected: engineering abbreviations appear only where the approved data context requires them; remove the English marketing kicker.

- [ ] **Step 7: Run all frontend gates**

```powershell
pnpm lint
pnpm typecheck
pnpm --filter @dicar/desktop exec vitest run
pnpm build
pnpm test:e2e
```

Expected: all commands exit 0.

- [ ] **Step 8: Commit E2E and responsive corrections**

```powershell
git add apps/dicar-desktop/e2e/initial-release.spec.ts apps/dicar-desktop/src/components/shell/AppShell.tsx apps/dicar-desktop/src/components/shell/ConnectionDrawer.tsx apps/dicar-desktop/src/pages/HomePage.tsx apps/dicar-desktop/src/pages/LiveWorkbenchPage.tsx apps/dicar-desktop/src/pages/RecordingsPage.tsx apps/dicar-desktop/src/pages/DiagnosticsPage.tsx apps/dicar-desktop/src/app/styles/tokens.css apps/dicar-desktop/src/app/styles/global.css
git commit -m "test(app): cover optimized desktop UI flows"
```

---

### Task 9: Documentation, backend-boundary proof, and final handoff

**Files:**
- Modify: `README.md`
- Modify: `docs/user-guide.md`
- Modify: `docs/development.md`
- Modify: `HANDOFF.md`

**Interfaces:**
- Documents the completed frontend UI only.
- Leaves wireless flashing implementation as the next sequential project.

- [ ] **Step 1: Update user-facing navigation and operation copy**

Document:

```markdown
- 顶部导航：概览、实时调试、波形记录、诊断。
- 点击设备状态芯片打开连接抽屉。
- 标准模式强调说明与安全上下文；赛道模式扩大波形和关键数值。
- 两种模式共享参数草稿、录制状态和实时数据。
- 波形记录库现在是独立页面。
- 无线烧录入口已预留但尚未启用。
```

Do not claim that wireless firmware flashing works.

- [ ] **Step 2: Update developer architecture and HANDOFF**

State that `workbenchMode` is a frontend-only settings v4 field; `ConnectionDrawer` still calls the unchanged `DesktopBridge`; `RecordingLibrary` reuses the existing controller; and `FirmwareFlashEntry` has no backend binding.

In `HANDOFF.md`, make “实现无线固件烧录后端与硬件流程” the next development item, followed by real nanoUART-wl/HC-05 hardware validation when hardware is available.

- [ ] **Step 3: Prove no backend file changed**

```powershell
$backendChanges = git diff --name-only 42b5687..HEAD | Select-String -Pattern '^(crates/|apps/dicar-desktop/src-tauri/)'
if ($backendChanges) {
  $backendChanges
  throw 'Frontend-only boundary violated'
}
'NO_BACKEND_CHANGES'
```

Expected: `NO_BACKEND_CHANGES`.

- [ ] **Step 4: Run final clean verification**

```powershell
pnpm lint
pnpm typecheck
pnpm --filter @dicar/desktop exec vitest run
pnpm build
pnpm test:e2e
git diff --check
git status --short
```

Expected: every frontend gate passes; `git diff --check` is clean; only the three intentionally untracked planning records may remain before the documentation commit.

- [ ] **Step 5: Commit documentation**

```powershell
git add README.md docs/user-guide.md docs/development.md HANDOFF.md
git commit -m "docs: document optimized desktop UI"
```

- [ ] **Step 6: Perform the final main-agent review**

Use `verification-before-completion`, then inspect:

```powershell
git log --oneline 42b5687..HEAD
git diff --stat 42b5687..HEAD
git status --short --branch
```

Confirm all nine task commits are present, no backend path appears, obsolete UI files are gone, the wireless entry remains truthful and disabled, and no release/version bump was performed. Stop here for review before starting the separate wireless-flashing design and implementation plan.
