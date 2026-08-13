import { act, fireEvent, render, screen } from "@testing-library/react";
import { App } from "../../app/App";
import { AppProviders } from "../../app/providers";
import { MockBridge } from "../../bridge/mockBridge";
import { useSettingsStore } from "../../stores/settingsStore";
import { useVehicleProfileStore } from "../../stores/vehicleProfileStore";

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

async function openWizard() {
  window.history.pushState({}, "", "/live/car-01");
  render(<AppProviders bridge={new MockBridge()}><App /></AppProviders>);
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
