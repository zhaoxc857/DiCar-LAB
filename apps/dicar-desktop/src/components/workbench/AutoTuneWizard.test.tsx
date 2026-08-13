import { act, fireEvent, render, screen } from "@testing-library/react";
import { App } from "../../app/App";
import { AppProviders } from "../../app/providers";
import { MockBridge } from "../../bridge/mockBridge";
import { useSettingsStore } from "../../stores/settingsStore";
import { useVehicleProfileStore } from "../../stores/vehicleProfileStore";
import { validateExperimentTargets } from "./AutoTuneWizard";

const TUNABLE_PROFILE = `schema_version: 1
vehicle: { id: autotune-car, display_name: 自动调参车, type: 测试, order: 50 }
control_loops:
  - id: speed
    label: 速度环
    target_parameter: control.target_speed_mps
    gains: { Kp: pid.kp }
    telemetry:
      target: drive.target_speed_mps
      feedback: drive.speed_mps
`;

beforeEach(() => {
  localStorage.clear();
  useVehicleProfileStore.getState().reset();
  useSettingsStore.getState().saveAiSettings("https://api.deepseek.com", "deepseek-chat", "");
});

afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
});

async function openWizard(bridge = new MockBridge()) {
  window.history.pushState({}, "", "/live/car-01");
  render(<AppProviders bridge={bridge}><App /></AppProviders>);
  await act(async () => undefined);
  fireEvent.click(screen.getByRole("button", { name: "连接模拟器" }));
  await screen.findByText("已就绪");
  fireEvent.click(screen.getByRole("button", { name: "AI 调参" }));
  return screen.getByRole("dialog", { name: "AI 自动调参" });
}

it("explains when the vehicle profile has no auto-tunable control loop", async () => {
  await openWizard();
  expect(screen.getByText(/没有可自动调参的控制环/)).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "开始自动调参" })).toBeDisabled();
});

it("requires an API key and gain selection before starting", async () => {
  act(() => {
    expect(useVehicleProfileStore.getState().importProfile(TUNABLE_PROFILE, false).status).toBe("imported");
    useVehicleProfileStore.getState().selectProfile("autotune-car");
  });
  await openWizard();

  expect(screen.getByText(/请先填写 DeepSeek API Key/)).toBeInTheDocument();
  fireEvent.change(screen.getByLabelText(/API Key/), { target: { value: "sk-test" } });
  expect(screen.getByText(/请至少勾选一个要整定的增益参数/)).toBeInTheDocument();

  fireEvent.click(screen.getByRole("checkbox", { name: /速度环 Kp/ }));
  expect(screen.getByRole("button", { name: "开始自动调参" })).toBeEnabled();
  expect(screen.getByText(/首轮实验请将车辆架空/)).toBeInTheDocument();
});

it("rejects non-finite, equal, and out-of-range experiment targets", () => {
  const target = {
    paramId: 4,
    machineName: "control.target_speed_mps",
    displayName: "目标速度",
    group: "速度环",
    unit: "m/s",
    ramValue: { kind: "f32" as const, value: 0 },
    persistedValue: null,
    revision: 1,
    dirty: false,
    syncKnown: true,
    writeState: "idle" as const,
    writable: true,
    dangerous: true,
    lastError: null,
    numeric: { min: 0, max: 8, step: 0.05 },
  };

  expect(validateExperimentTargets(target, Number.NaN, 1)).toMatch(/有限数值/);
  expect(validateExperimentTargets(target, 1, 1)).toMatch(/必须不同/);
  expect(validateExperimentTargets(target, -1, 1)).toMatch(/0–8/);
  expect(validateExperimentTargets(target, 0, 9)).toMatch(/0–8/);
  expect(validateExperimentTargets(target, 0, 1)).toBeNull();
});

it("restores the original target and subscription after an AI failure", async () => {
  vi.stubGlobal("fetch", vi.fn().mockResolvedValue({
    ok: false,
    status: 500,
    text: () => Promise.resolve("test failure"),
  }));
  act(() => {
    useVehicleProfileStore.getState().importProfile(TUNABLE_PROFILE, false);
    useVehicleProfileStore.getState().selectProfile("autotune-car");
  });
  const bridge = new MockBridge();
  await openWizard(bridge);
  await act(async () => {
    await bridge.writeParameter(4, { kind: "f32", value: 0.5 });
    await bridge.setTelemetrySubscription({ channelIds: [200], sampleRateHz: 250 });
    await bridge.setPaused(true);
  });
  vi.useFakeTimers();

  fireEvent.change(screen.getByLabelText(/API Key/), { target: { value: "sk-test" } });
  fireEvent.click(screen.getByRole("checkbox", { name: /速度环 Kp/ }));
  fireEvent.change(screen.getByLabelText(/每轮保持时长/), { target: { value: "1000" } });
  fireEvent.click(screen.getByRole("button", { name: "开始自动调参" }));
  await act(async () => {
    await vi.advanceTimersByTimeAsync(2_000);
  });

  expect(screen.getAllByText(/AI 决策失败/)).toHaveLength(2);
  const restored = await bridge.getSnapshot();
  expect(restored.parameters.find(({ paramId }) => paramId === 4)?.ramValue).toEqual({ kind: "f32", value: 0.5 });
  expect(restored.desiredSubscription).toMatchObject({ channelIds: [200], sampleRateHz: 250 });
  expect(restored.activeSubscription).toBeNull();
  expect(restored.paused).toBe(true);
});

it("clears the experiment subscription when none existed before the run", async () => {
  vi.stubGlobal("fetch", vi.fn().mockResolvedValue({
    ok: false,
    status: 500,
    text: () => Promise.resolve("test failure"),
  }));
  act(() => {
    useVehicleProfileStore.getState().importProfile(TUNABLE_PROFILE, false);
    useVehicleProfileStore.getState().selectProfile("autotune-car");
  });
  const bridge = new MockBridge();
  const clearSpy = vi.spyOn(bridge, "clearTelemetrySubscription");
  await openWizard(bridge);
  await act(async () => {
    await bridge.clearTelemetrySubscription();
  });
  clearSpy.mockClear();
  vi.useFakeTimers();

  fireEvent.change(screen.getByLabelText(/API Key/), { target: { value: "sk-test" } });
  fireEvent.click(screen.getByRole("checkbox", { name: /速度环 Kp/ }));
  fireEvent.change(screen.getByLabelText(/每轮保持时长/), { target: { value: "1000" } });
  fireEvent.click(screen.getByRole("button", { name: "开始自动调参" }));
  await act(async () => {
    await vi.advanceTimersByTimeAsync(2_000);
  });

  expect(screen.getAllByText(/AI 决策失败/)).toHaveLength(2);
  expect(clearSpy).toHaveBeenCalledTimes(1);
  expect(await bridge.getSnapshot()).toMatchObject({
    desiredSubscription: null,
    activeSubscription: null,
    paused: true,
  });
});

it("reports a rejected cleanup operation and still reaches the done phase", async () => {
  vi.stubGlobal("fetch", vi.fn().mockResolvedValue({
    ok: false,
    status: 500,
    text: () => Promise.resolve("test failure"),
  }));
  act(() => {
    useVehicleProfileStore.getState().importProfile(TUNABLE_PROFILE, false);
    useVehicleProfileStore.getState().selectProfile("autotune-car");
  });
  const bridge = new MockBridge();
  await openWizard(bridge);
  await act(async () => {
    await bridge.clearTelemetrySubscription();
  });
  vi.spyOn(bridge, "clearTelemetrySubscription").mockRejectedValueOnce(new Error("cleanup exploded"));
  vi.useFakeTimers();

  fireEvent.change(screen.getByLabelText(/API Key/), { target: { value: "sk-test" } });
  fireEvent.click(screen.getByRole("checkbox", { name: /速度环 Kp/ }));
  fireEvent.change(screen.getByLabelText(/每轮保持时长/), { target: { value: "1000" } });
  fireEvent.click(screen.getByRole("button", { name: "开始自动调参" }));
  await act(async () => {
    await vi.advanceTimersByTimeAsync(2_000);
  });

  expect(screen.getAllByText(/清除实验订阅失败：cleanup exploded/)).toHaveLength(2);
  expect(screen.getByRole("button", { name: "完成" })).toBeInTheDocument();
});

it("reports a rejected pre-run snapshot without remaining stuck", async () => {
  act(() => {
    useVehicleProfileStore.getState().importProfile(TUNABLE_PROFILE, false);
    useVehicleProfileStore.getState().selectProfile("autotune-car");
  });
  const bridge = new MockBridge();
  await openWizard(bridge);
  vi.spyOn(bridge, "getSnapshot").mockRejectedValueOnce(new Error("snapshot unavailable"));

  fireEvent.change(screen.getByLabelText(/API Key/), { target: { value: "sk-test" } });
  fireEvent.click(screen.getByRole("checkbox", { name: /速度环 Kp/ }));
  fireEvent.click(screen.getByRole("button", { name: "开始自动调参" }));

  expect(await screen.findAllByText(/读取实验前状态失败：snapshot unavailable/)).toHaveLength(2);
  expect(screen.getByRole("button", { name: "完成" })).toBeInTheDocument();
});
