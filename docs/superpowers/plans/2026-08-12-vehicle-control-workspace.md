# Vehicle Control Workspace Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add safe YAML vehicle profiles that resolve against DCTP Manifest truth and drive reusable control-loop, parameter-section, and recommended-waveform workspaces in the existing Windows App.

**Architecture:** A bounded pure parser turns YAML text into schema-v1 profiles, and a pure resolver binds stable machine names to the current `AppSnapshot` descriptors. A persisted profile store owns only local profile text and selection; `LiveWorkbenchPage` consumes resolved IDs, while `WaveformPanel` accepts one-shot pending-selection requests without changing the device subscription automatically.

**Tech Stack:** React 19, TypeScript 5.9, Zustand persist, `yaml` 2.x, Vitest, Testing Library, Playwright, Vite raw assets, Tauri 2.

## Global Constraints

- DCTP Manifest remains the only truth for parameter type, range, unit, writable/dangerous flags, Revision, RAM/Flash values, and telemetry descriptors.
- YAML cannot create arbitrary DCTP commands, override device constraints, or bypass Owner/Tuner/Observer and lease rules.
- Schema version is exactly `1`; one file is at most 256 KiB, at most 16 user profiles and 2 MiB total, at most 32 loops/sections/presets, and at most 64 references per list.
- YAML custom tags, anchors, aliases, and merge keys are rejected.
- Missing or incompatible profile fields never block connection, diagnostics, all-parameter editing, or the generic Manifest waveform workspace.
- Selecting a control loop or preset changes only pending waveform channels; only the existing Apply action calls `setTelemetrySubscription`.
- Keep the existing 8-channel/link-budget limits, 30,000 points per channel, pixel min/max downsampling, and <=30 visual revisions per second.
- This plan does not modify DCTP, Rust `AppActor`, or the `DesktopBridge` command contract.
- Use strict RED -> GREEN -> REFACTOR; every production behavior is preceded by a test observed failing for the intended missing behavior.

---

## File Structure

- Create `apps/dicar-desktop/src/vehicleProfiles/types.ts`: schema-v1 input and resolved-workspace types only.
- Create `apps/dicar-desktop/src/vehicleProfiles/parser.ts`: bounded YAML parse plus structural validation.
- Create `apps/dicar-desktop/src/vehicleProfiles/parser.test.ts`: literal valid/malformed/adversarial YAML tests.
- Create `apps/dicar-desktop/src/vehicleProfiles/resolver.ts`: exact machine-name binding and compatibility issues.
- Create `apps/dicar-desktop/src/vehicleProfiles/resolver.test.ts`: independent Manifest DTO fixtures and literal resolved IDs.
- Create `apps/dicar-desktop/src/vehicleProfiles/builtins/dicar-diff-drive.yaml`: packaged reference profile compatible with the real simulator Manifest subset.
- Create `apps/dicar-desktop/src/stores/vehicleProfileStore.ts`: selected profile and bounded user-profile persistence.
- Create `apps/dicar-desktop/src/stores/vehicleProfileStore.test.ts`: import/replace/remove/migration behavior.
- Modify `apps/dicar-desktop/src/stores/settingsStore.ts`: remove the obsolete placeholder `vehicleId` after migration.
- Modify `apps/dicar-desktop/src/components/shell/VehicleSwitcher.tsx`: real profile selection and manager entry.
- Create `apps/dicar-desktop/src/components/vehicleProfiles/VehicleProfileManager.tsx`: file import, replace confirmation, removal, and compatibility report.
- Create `apps/dicar-desktop/src/components/workbench/WorkspaceNav.tsx`: loops, sections, and generic/all-parameter tasks.
- Create `apps/dicar-desktop/src/components/workbench/ControlLoopWorkspace.tsx`: resolved role values and existing typed parameter controls.
- Modify `apps/dicar-desktop/src/pages/LiveWorkbenchPage.tsx`: resolve the active profile and own task/waveform request state.
- Modify `apps/dicar-desktop/src/components/workbench/WaveformPanel.tsx`: consume each external pending selection exactly once.
- Modify `apps/dicar-desktop/src/telemetry/telemetryWorkgroups.ts`: merge explicit profile workgroups with deterministic automatic groups.
- Modify focused unit/component/E2E tests and package metadata.

### Task 1: Bounded YAML Schema Parser

**Files:**
- Create: `apps/dicar-desktop/src/vehicleProfiles/types.ts`
- Create: `apps/dicar-desktop/src/vehicleProfiles/parser.ts`
- Create: `apps/dicar-desktop/src/vehicleProfiles/parser.test.ts`
- Modify: `apps/dicar-desktop/package.json`
- Modify: `pnpm-lock.yaml`
- Modify: `apps/dicar-desktop/tsconfig.app.json`

**Interfaces:**
- Produces `VehicleProfileV1`, `VehicleControlLoop`, `VehicleParameterSection`, `VehicleScopePreset`, and `VehicleProfileParseError`.
- Produces `parseVehicleProfile(text: string): VehicleProfileV1`.
- `VehicleProfileParseError` exposes a stable Chinese `message` and `path` but never includes the full imported file.

- [x] **Step 1: Add the explicit YAML dependency and Vite raw-asset typing**

Run:

```powershell
pnpm --filter @dicar/desktop add yaml@^2.8.1
```

Then add `"vite/client"` to `compilerOptions.types` in `tsconfig.app.json`. Do not rely on the transitive lockfile entry.

- [x] **Step 2: Write literal failing parser tests**

Create tests whose expectations are hand-written rather than derived from parser helpers:

```ts
it("parses one bounded control loop without inventing defaults", () => {
  expect(parseVehicleProfile(`
schema_version: 1
vehicle: { id: dicar_diff, display_name: DiCar 差速车, type: 两轮差速, order: 10 }
control_loops:
  - id: speed
    label: 速度环
    gains: { Kp: pid.kp }
    telemetry: { feedback: drive.speed_mps, outputs: [motor.left_pwm] }
`)).toMatchObject({
    schemaVersion: 1,
    vehicle: { id: "dicar_diff", displayName: "DiCar 差速车", type: "两轮差速", order: 10 },
    controlLoops: [{ id: "speed", label: "速度环", gains: { Kp: "pid.kp" } }],
  });
});

it.each([
  ["unknown schema", "schema_version: 2\nvehicle: {id: x, display_name: X, type: X, order: 1}", "schema_version"],
  ["duplicate loop id", VALID_WITH_DUPLICATE_LOOPS, "control_loops[1].id"],
  ["alias", VALID_WITH_ALIAS, "YAML 别名"],
  ["merge key", VALID_WITH_MERGE_KEY, "YAML merge key"],
])("rejects %s", (_name, text, message) => {
  expect(() => parseVehicleProfile(text)).toThrow(message);
});

it("rejects text above 256 KiB before parsing it", () => {
  expect(() => parseVehicleProfile("x".repeat(256 * 1024 + 1))).toThrow("256 KiB");
});
```

Also cover illegal IDs, duplicate references, more than 32 loops and a 65-channel preset. The production changes caught are missing bounds, wrong path reporting, and unsafe YAML graph features.

- [x] **Step 3: Run RED and confirm the missing parser failure**

Run: `pnpm --filter @dicar/desktop test -- src/vehicleProfiles/parser.test.ts --run`

Expected: FAIL because `parser.ts`/exports do not exist, not because a fixture has invalid TypeScript.

- [x] **Step 4: Implement the minimum safe parser**

Use `parseDocument(text, { uniqueKeys: true, maxAliasCount: 0 })`, traverse the parsed document to reject nodes with `anchor`, aliases and `<<`, then map only known schema fields into camelCase typed values. Reject unknown top-level keys so a misspelled `control_loop` cannot silently disappear; allow unknown future fields only inside a documented `metadata` object. Implement reusable `expectObject`, `expectString`, `expectArray`, `expectId`, `uniqueIds`, and `uniqueReferences` helpers with exact path arguments.

- [x] **Step 5: Run GREEN and static gates**

Run:

```powershell
pnpm --filter @dicar/desktop test -- src/vehicleProfiles/parser.test.ts --run
pnpm --filter @dicar/desktop lint
pnpm --filter @dicar/desktop typecheck
```

Expected: parser tests pass; lint/typecheck exit 0 without suppressions.

- [x] **Step 6: Commit the parser slice**

```powershell
git add apps/dicar-desktop/package.json pnpm-lock.yaml apps/dicar-desktop/tsconfig.app.json apps/dicar-desktop/src/vehicleProfiles
git commit -m "feat(app): parse bounded vehicle profiles"
```

### Task 2: Manifest Resolver and Generic Fallback

**Files:**
- Create: `apps/dicar-desktop/src/vehicleProfiles/resolver.ts`
- Create: `apps/dicar-desktop/src/vehicleProfiles/resolver.test.ts`
- Modify: `apps/dicar-desktop/src/vehicleProfiles/types.ts`

**Interfaces:**
- Consumes `VehicleProfileV1`, `ParameterSnapshot[]`, and `TelemetryDescriptor[]`.
- Produces `CompatibilityIssue = { severity: "error" | "warning" | "info"; path: string; message: string }`.
- Produces `ResolvedControlLoop`, `ResolvedParameterSection`, `ResolvedScopePreset`, and `ResolvedVehicleWorkspace` containing only resolved `paramId`/`channelId` plus display metadata.
- Produces `resolveVehicleWorkspace(profile, parameters, telemetry): ResolvedVehicleWorkspace`.
- Produces `genericVehicleWorkspace(parameters, telemetry): ResolvedVehicleWorkspace` with profile ID `generic-manifest`.

- [x] **Step 1: Write failing resolver tests with independent DTO fixtures**

```ts
it("binds exact parameter and telemetry machine names to stable numeric IDs", () => {
  const resolved = resolveVehicleWorkspace(profile, [kp, readOnlyTarget], [target, speed, pwm]);
  expect(resolved.controlLoops[0]).toMatchObject({
    id: "speed",
    gainParamIds: [{ label: "Kp", paramId: 1 }],
    targetParamId: 4,
    telemetry: { target: 207, feedback: 200, outputs: [209] },
    recommendedChannelIds: [207, 200, 209],
  });
});

it("keeps a read-only target visible but reports and disables target writing", () => {
  const resolved = resolveVehicleWorkspace(profile, [kp, readOnlyTarget], [speed]);
  expect(resolved.controlLoops[0].targetWritable).toBe(false);
  expect(resolved.issues).toContainEqual(expect.objectContaining({ severity: "warning", path: "control_loops[0].target_parameter" }));
});

it("drops missing recommendations, keeps valid roles, and falls back only when no task remains", () => {
  const partial = resolveVehicleWorkspace(profile, [kp], [speed]);
  expect(partial.controlLoops).toHaveLength(1);
  expect(partial.controlLoops[0].recommendedChannelIds).toEqual([200]);
  expect(resolveVehicleWorkspace(emptyProfile, [], []).fallbackRequired).toBe(true);
});
```

Cover wrong parameter kind for gain, a telemetry name used as a parameter, section omissions, preset ordering/de-duplication, and all unreferenced parameters remaining available through the generic task.

- [x] **Step 2: Run RED**

Run: `pnpm --filter @dicar/desktop test -- src/vehicleProfiles/resolver.test.ts --run`

Expected: FAIL for missing resolver exports.

- [x] **Step 3: Implement indexed exact binding**

Build one `Map` for parameter machine names and one for telemetry machine names per resolve. Never scan by display label and never mutate the DTOs. Treat `f32`, `i32`, `u32`, and `enum` as numeric for gain display, but require target parameters to be writable and non-boolean before enabling writes. Preserve valid partial loops and emit deterministic issues in schema order.

- [x] **Step 4: Run GREEN and mutation checks**

Run the focused tests, then temporarily verify that changing exact match to case-insensitive makes the case-mismatch test fail; restore exact matching and rerun:

```powershell
pnpm --filter @dicar/desktop test -- src/vehicleProfiles/resolver.test.ts --run
pnpm --filter @dicar/desktop typecheck
```

- [x] **Step 5: Commit resolver behavior**

```powershell
git add apps/dicar-desktop/src/vehicleProfiles
git commit -m "feat(app): resolve vehicle profiles against manifests"
```

### Task 3: Built-In and Persisted User Profiles

**Files:**
- Create: `apps/dicar-desktop/src/vehicleProfiles/builtins/dicar-diff-drive.yaml`
- Create: `apps/dicar-desktop/src/vehicleProfiles/catalog.ts`
- Create: `apps/dicar-desktop/src/stores/vehicleProfileStore.ts`
- Create: `apps/dicar-desktop/src/stores/vehicleProfileStore.test.ts`
- Modify: `apps/dicar-desktop/src/stores/settingsStore.ts`
- Test: `apps/dicar-desktop/src/stores/vehicleProfileStore.test.ts` also covers migration from the persisted `dicar-tune-settings` legacy shape; do not create a second migration test file.

**Interfaces:**
- Produces `GENERIC_PROFILE_ID = "generic-manifest"` and `builtInProfiles: StoredVehicleProfile[]`.
- Produces `StoredVehicleProfile = { source: "builtIn" | "user"; profile: VehicleProfileV1; yamlText: string }`.
- Store state: `{ selectedProfileId; userProfiles; importProfile(yamlText, replaceExisting): ImportProfileResult; removeUserProfile(id); selectProfile(id); reset() }`.
- `ImportProfileResult` is a tagged result: `imported`, `needsReplace`, or `failed`, with Chinese message and no thrown UI exception.

- [x] **Step 1: Write RED tests for catalog/store contracts**

```ts
it("packages a built-in profile that resolves useful simulator tasks", () => {
  const builtIn = builtInProfiles.find(({ profile }) => profile.vehicle.id === "dicar-diff-drive");
  const resolved = resolveVehicleWorkspace(builtIn!.profile, simulatorParameters, simulatorTelemetry);
  expect(resolved.controlLoops.map(({ id }) => id)).toEqual(["speed"]);
  expect(resolved.controlLoops[0].recommendedChannelIds).toEqual([207, 200, 208, 209, 210]);
});

it("requires explicit replacement and never lets a user shadow a built-in id", () => {
  expect(store.importProfile(USER_YAML, false).status).toBe("imported");
  expect(store.importProfile(UPDATED_USER_YAML, false).status).toBe("needsReplace");
  expect(store.importProfile(UPDATED_USER_YAML, true).status).toBe("imported");
  expect(store.importProfile(BUILTIN_ID_YAML, true).status).toBe("failed");
});

it("removing the active user profile falls back to generic", () => {
  store.importProfile(USER_YAML, false); store.selectProfile("user-car"); store.removeUserProfile("user-car");
  expect(store.selectedProfileId).toBe("generic-manifest");
});
```

Also test 16-profile/2-MiB limits and migration of legacy persisted `vehicleId: "car-01"` to `generic-manifest` without changing serial settings.

- [x] **Step 2: Run RED**

Run: `pnpm --filter @dicar/desktop test -- src/stores/vehicleProfileStore.test.ts --run`

Expected: FAIL because catalog/store do not exist.

- [x] **Step 3: Add the real built-in YAML and store**

The YAML must target real simulator names (`pid.kp`, `drive.speed_mps`, `drive.target_speed_mps`, `drive.speed_error_mps`, `motor.left_pwm`, `motor.right_pwm`, encoder names, and `drive.wheel_diameter_mm`). Import it with Vite `?raw`, parse it using Task 1, and fail module initialization loudly if an in-repo built-in is invalid. Persist only selected ID and user YAML text; reparse persisted text and discard malformed entries with a recoverable catalog issue.

Replace the obsolete `settingsStore.vehicleId` with profile-store selection. Keep serial hardware fields untouched.

- [x] **Step 4: Run GREEN, then all store and parser/resolver tests**

```powershell
pnpm --filter @dicar/desktop test -- src/stores/vehicleProfileStore.test.ts src/vehicleProfiles --run
pnpm --filter @dicar/desktop lint
pnpm --filter @dicar/desktop typecheck
```

- [x] **Step 5: Commit profile persistence**

```powershell
git add apps/dicar-desktop/src/vehicleProfiles apps/dicar-desktop/src/stores apps/dicar-desktop/tsconfig.app.json
git commit -m "feat(app): persist vehicle profile catalog"
```

### Task 4: Profile Selection, Management, and Workspace UI

**Files:**
- Modify: `apps/dicar-desktop/src/components/shell/VehicleSwitcher.tsx`
- Create: `apps/dicar-desktop/src/components/vehicleProfiles/VehicleProfileManager.tsx`
- Create: `apps/dicar-desktop/src/components/vehicleProfiles/VehicleProfileManager.test.tsx`
- Create: `apps/dicar-desktop/src/components/workbench/WorkspaceNav.tsx`
- Create: `apps/dicar-desktop/src/components/workbench/ControlLoopWorkspace.tsx`
- Create: `apps/dicar-desktop/src/components/workbench/ControlLoopWorkspace.test.tsx`
- Modify: `apps/dicar-desktop/src/pages/LiveWorkbenchPage.tsx`
- Modify: `apps/dicar-desktop/src/pages/LiveWorkbenchPage.test.tsx`
- Modify: `apps/dicar-desktop/src/components/workbench/ParameterEditor.tsx`

**Interfaces:**
- `VehicleProfileManager` reads files with `File.text()` from an `<input type="file" accept=".yaml,.yml">`; it never calls a new Bridge command.
- `WorkspaceTask = { kind: "loop" | "section" | "group" | "all"; id: string }`.
- `WorkspaceNav` receives resolved workspace, all parameters, selected task, and `onSelectTask`.
- `ControlLoopWorkspace` receives one `ResolvedControlLoop`, parameter records, telemetry descriptors, and the shared ring buffer.

- [x] **Step 1: Write failing manager and workbench tests**

```tsx
it("imports a profile through the real file input and requires confirmation before replacement", async () => {
  render(<VehicleProfileManager open onClose={() => undefined} />);
  const file = new File([USER_YAML], "user-car.yaml", { type: "application/yaml" });
  fireEvent.change(screen.getByLabelText("导入车型 YAML"), { target: { files: [file] } });
  expect(await screen.findByText("已导入 用户车")).toBeInTheDocument();
  fireEvent.change(screen.getByLabelText("导入车型 YAML"), { target: { files: [new File([UPDATED_USER_YAML], "user-car.yaml")] } });
  expect(await screen.findByRole("button", { name: "确认替换 用户车" })).toBeInTheDocument();
});

it("renders a resolved speed loop without bypassing existing typed parameter controls", async () => {
  renderAppAtLivePage();
  await connectSimulator();
  fireEvent.click(screen.getByRole("button", { name: "速度环" }));
  expect(screen.getByText("目标")).toBeInTheDocument();
  expect(screen.getByText("实际")).toBeInTheDocument();
  expect(screen.getByText("误差")).toBeInTheDocument();
  expect(screen.getByLabelText("速度 Kp")).toBeInTheDocument();
  expect(screen.getByText(/设备清单未提供可写目标参数/)).toBeInTheDocument();
});

it("keeps all parameters reachable when a selected profile is partially incompatible", async () => {
  selectIncompatibleUserProfile();
  renderAppAtLivePage();
  await connectSimulator();
  expect(screen.getByText(/兼容性/)).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "全部参数" }));
  expect(screen.getByLabelText(/PPR|Kp/)).toBeInTheDocument();
});
```

The tests must use the real `MockBridge`, stores and components. Only spy on a Bridge method when validating a device-side call; never replace the workspace with a mock component.

- [x] **Step 2: Run RED**

Run: `pnpm --filter @dicar/desktop test -- src/components/vehicleProfiles src/components/workbench/ControlLoopWorkspace.test.tsx src/pages/LiveWorkbenchPage.test.tsx --run`

Expected: FAIL for missing manager/nav/loop UI and old placeholder switcher.

- [x] **Step 3: Implement the minimal accessible UI**

Use Radix Dialog for management. The top selector labels itself “车型配置”, always includes “通用 Manifest”, and orders profiles by `order` then display name. Show source and compatibility issue counts in the manager. Use semantic buttons in `WorkspaceNav`; preserve the existing search/modified-only `ParameterNav` behavior under group/all tasks rather than removing it.

`ControlLoopWorkspace` renders role cards from buffer latest points and reuses `TypedParameterControl` for every resolved target/gain record. It never calculates a wider range or sends automatically. Reuse `EncoderCalibrationPanel` only when a resolved section includes all three required encoder baseline names; otherwise use ordinary typed controls plus compatibility text.

- [x] **Step 4: Run GREEN and existing parameter regressions**

```powershell
pnpm --filter @dicar/desktop test -- src/components/vehicleProfiles src/components/workbench src/pages/LiveWorkbenchPage.test.tsx --run
pnpm --filter @dicar/desktop lint
pnpm --filter @dicar/desktop typecheck
```

- [x] **Step 5: Commit workspace UI**

```powershell
git add apps/dicar-desktop/src/components apps/dicar-desktop/src/pages/LiveWorkbenchPage.tsx apps/dicar-desktop/src/pages/LiveWorkbenchPage.test.tsx
git commit -m "feat(app): add manifest-safe control workspaces"
```

### Task 5: One-Shot Recommended Waveform Selection

**Files:**
- Modify: `apps/dicar-desktop/src/telemetry/telemetryWorkgroups.ts`
- Modify: `apps/dicar-desktop/src/telemetry/telemetryWorkgroups.test.ts`
- Modify: `apps/dicar-desktop/src/components/workbench/WaveformPanel.tsx`
- Modify: `apps/dicar-desktop/src/components/workbench/WaveformPanel.test.tsx`
- Modify: `apps/dicar-desktop/src/pages/LiveWorkbenchPage.tsx`
- Modify: `apps/dicar-desktop/src/pages/LiveWorkbenchPage.test.tsx`

**Interfaces:**
- Produces `WaveformSelectionRequest = { requestId: number; label: string; channelIds: number[] }`.
- `WaveformPanel` gains optional `selectionRequest?: WaveformSelectionRequest | null` and `profileWorkgroups?: TelemetryWorkgroup[]` props.
- Produces `mergeTelemetryWorkgroups(profileGroups, automaticGroups): TelemetryWorkgroup[]`, rejecting duplicate IDs and de-duplicating missing channel IDs against the current descriptor set at the component boundary.

- [x] **Step 1: Write failing one-shot behavior tests**

```tsx
it("applies each external request once and does not overwrite later manual channel choices", async () => {
  const { rerender } = renderWaveform({ requestId: 1, label: "速度环推荐", channelIds: [207, 200, 208, 209, 210] });
  expect(screen.getByText("5/8 通道")).toBeInTheDocument();
  openChannels(); toggleChannel("右 PWM");
  rerenderWaveform({ requestId: 1, label: "速度环推荐", channelIds: [207, 200, 208, 209, 210] });
  expect(screen.getByText("4/8 通道")).toBeInTheDocument();
  rerenderWaveform({ requestId: 2, label: "速度环推荐", channelIds: [207, 200, 208] });
  expect(screen.getByText("3/8 通道")).toBeInTheDocument();
});

it("clips a loop recommendation to the active link budget without subscribing", async () => {
  const setSubscription = vi.spyOn(bridge, "setTelemetrySubscription");
  renderHc05Waveform({ requestId: 1, label: "驱动总览", channelIds: [200, 205, 206, 207, 208] });
  expect(screen.getByText("当前链路已保留 4 个通道，省略 1 个")).toBeInTheDocument();
  expect(setSubscription).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "应用 50 Hz 订阅" }));
  await waitFor(() => expect(setSubscription).toHaveBeenCalledWith({ channelIds: [200, 205, 206, 207], sampleRateHz: 50 }));
});
```

Also test profile presets before automatic groups, ID collisions, and a request whose channels all disappeared after Manifest change.

- [x] **Step 2: Run RED**

Run: `pnpm --filter @dicar/desktop test -- src/telemetry/telemetryWorkgroups.test.ts src/components/workbench/WaveformPanel.test.tsx src/pages/LiveWorkbenchPage.test.tsx --run`

Expected: FAIL because the props/request effect and merge function are absent.

- [x] **Step 3: Implement request consumption without feedback loops**

Track `lastConsumedRequestId` in a ref. When a new request arrives, filter to current descriptor IDs, preserve request order, clip through the existing max-channel path, leave fixed Y mode, and update pending channels. Do not include `selectedIds` in an effect that can replay the same request. Manual selection remains `custom` until a newer request or explicit toolbar workgroup selection.

`LiveWorkbenchPage` increments the request ID only in the task-selection event handler. Resolving a new AppSnapshot does not generate a request unless the previously selected task becomes invalid and the page must select a new task.

- [x] **Step 4: Run GREEN and telemetry capacity regressions**

```powershell
pnpm --filter @dicar/desktop test -- src/telemetry src/components/workbench/WaveformPanel.test.tsx src/stores/workspaceStore.test.ts --run
pnpm --filter @dicar/desktop lint
pnpm --filter @dicar/desktop typecheck
```

- [x] **Step 5: Commit waveform linkage**

```powershell
git add apps/dicar-desktop/src/telemetry apps/dicar-desktop/src/components/workbench/WaveformPanel* apps/dicar-desktop/src/pages/LiveWorkbenchPage*
git commit -m "feat(app): link control tasks to pending waveforms"
```

### Task 6: App Acceptance, Packaging, and Documentation

**Files:**
- Modify: `apps/dicar-desktop/e2e/initial-release.spec.ts`
- Modify: `README.md`
- Modify: `docs/development.md`
- Modify: `docs/user-guide.md`
- Modify: `docs/superpowers/plans/2026-08-12-vehicle-control-workspace.md` to check completed steps only after evidence exists

**Interfaces:** No new production API; this task validates the complete integrated App.

- [x] **Step 1: Add acceptance tests before any acceptance-only fixes**

Add Playwright coverage that uses the built-in profile and the real page:

```ts
test("车型速度环组织参数并只在确认后应用推荐波形", async ({ page }) => {
  await page.goto("/live/car-01");
  await page.getByLabel("车型配置").selectOption("dicar-diff-drive");
  await page.getByRole("button", { name: "连接模拟器" }).click();
  await page.getByRole("button", { name: "速度环" }).click();
  await expect(page.getByText("实际", { exact: true })).toBeVisible();
  await expect(page.getByText("误差", { exact: true })).toBeVisible();
  await expect(page.getByText("5/8 通道", { exact: true })).toBeVisible();
  await page.getByRole("button", { name: /应用 500 Hz 订阅/ }).click();
  await expect(page.getByText(/5\/8 通道/)).toBeVisible();
});
```

Add a 640x360 effective-200%-zoom assertion that the profile selector, workspace nav, one parameter action, waveform toolbar and table remain reachable without document horizontal overflow. Add keyboard navigation through profile selector -> task -> parameter -> waveform region. If these pass immediately, record that existing implementation already meets acceptance and do not manufacture a failure.

- [x] **Step 2: Run E2E and fix only observed acceptance gaps**

Run: `pnpm --filter @dicar/desktop test:e2e`

Expected before fixes: new flow may fail on actual accessible names/layout; existing four flows must remain green. Any fix needs a focused unit or E2E assertion that would fail if reverted.

- [x] **Step 3: Document the user and contributor contracts**

Document where built-ins live, the complete schema-v1 example, import/replace/remove behavior, Manifest exact-name binding, compatibility severity, size/count limits, generic fallback, and the prohibition against overriding device truth. Do not promise remote sharing, CMD, auto PID, schemes, experiments, or corner analysis.

- [x] **Step 4: Run the fresh full verification matrix**

From the worktree root with bundled Node/pnpm on PATH:

```powershell
pnpm lint
pnpm typecheck
pnpm test -- --run
pnpm build
pnpm test:e2e
git diff --check
```

Then run the native package gate because a raw YAML asset must be proven present in the installed bundle:

```powershell
pnpm --filter @dicar/desktop tauri:build
```

Expected: every command exits 0, all existing plus new tests pass, and Tauri produces the NSIS installer. Inspect the built App using the built-in profile; do not claim packaging from a Vite-only build.

- [x] **Step 5: Inspect complete scope and commit acceptance**

Inspect `git diff 4c6f790..HEAD`, `git status --short`, generated bundle paths, dependency additions, placeholders, unbounded collections, forbidden arbitrary command paths, and accidental Manifest/DCTP changes. Then commit:

```powershell
git add apps/dicar-desktop/e2e README.md docs/development.md docs/user-guide.md docs/superpowers/plans/2026-08-12-vehicle-control-workspace.md
git commit -m "test(app): verify vehicle control workspaces"
```

Do not stage generated `dist`, `target`, Playwright reports, or installer output. Record installer path/hash in the progress log only after the native build succeeds.
