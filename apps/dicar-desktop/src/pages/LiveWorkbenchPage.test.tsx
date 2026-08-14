import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { App } from "../app/App";
import { AppProviders } from "../app/providers";
import { MockBridge } from "../bridge/mockBridge";
import { useVehicleProfileStore } from "../stores/vehicleProfileStore";

beforeEach(() => useVehicleProfileStore.getState().reset());

it("runs the B-to-A tuning flow with permissions, RAM truth, and commit review", async () => {
  window.history.pushState({}, "", "/live/car-01");
  const bridge = new MockBridge();
  render(<AppProviders bridge={bridge}><App /></AppProviders>);
  await act(async () => undefined);

  expect(screen.getByRole("heading", { name: "实时调参与波形" })).toBeInTheDocument();
  expect(screen.getByRole("heading", { name: "参数目录" })).toBeInTheDocument();
  expect(screen.getByRole("heading", { name: "实时波形" })).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "波形记录" }));
  expect(await screen.findByRole("dialog", { name: "波形记录库" })).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "关闭波形记录库" }));
  expect(screen.getByText("本地演示权限，不是远程安全边界")).toBeInTheDocument();

  fireEvent.click(screen.getByRole("button", { name: "连接模拟器" }));
  expect(await screen.findByText("已就绪")).toBeInTheDocument();
  expect(screen.getByText(/19 个设备参数/)).toBeInTheDocument();
  expect(screen.getByText("8/8 通道")).toBeInTheDocument();

  fireEvent.click(screen.getByRole("button", { name: /速度环 PID/ }));
  fireEvent.change(screen.getByLabelText("速度环 Kp"), { target: { value: "1.8" } });
  fireEvent.click(screen.getByRole("button", { name: "写入 RAM" }));
  await waitFor(() => expect(screen.getByText("1 项待固化")).toBeInTheDocument());

  fireEvent.click(screen.getByRole("button", { name: "审阅并固化" }));
  expect(screen.getByRole("dialog", { name: "固化参数修改" })).toBeInTheDocument();
  expect(screen.getByRole("cell", { name: "速度环 Kp" })).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "固化到 Flash" }));
  await waitFor(() => expect(screen.getByText("0 项待固化")).toBeInTheDocument());
});

it("keeps Observer read-only with a textual denial reason", async () => {
  window.history.pushState({}, "", "/live/car-01");
  const bridge = new MockBridge();
  render(<AppProviders bridge={bridge}><App /></AppProviders>);
  await act(async () => undefined);
  fireEvent.click(screen.getByRole("button", { name: "连接模拟器" }));
  await screen.findByText("已就绪");

  fireEvent.change(screen.getByLabelText("演示身份"), { target: { value: "observer" } });
  await screen.findByText("仅观察者不能修改参数");
  expect(screen.queryByRole("button", { name: "写入 RAM" })).not.toBeInTheDocument();
  expect(screen.getByRole("button", { name: "审阅并固化" })).toBeDisabled();
});

it("organizes the simulator as a vehicle speed-control workspace", async () => {
  useVehicleProfileStore.getState().selectProfile("dicar-diff-drive");
  window.history.pushState({}, "", "/live/car-01");
  render(<AppProviders bridge={new MockBridge()}><App /></AppProviders>);
  await act(async () => undefined);
  fireEvent.click(screen.getByRole("button", { name: "连接模拟器" }));
  await screen.findByText("已就绪");
  fireEvent.click(screen.getByRole("button", { name: "速度环" }));
  expect(screen.getByText("目标", { exact: true })).toBeInTheDocument();
  expect(screen.getByLabelText("目标速度")).toBeInTheDocument();
  expect(screen.getByLabelText("速度环 Kp")).toBeInTheDocument();
  expect(screen.getByLabelText("速度环 Ki")).toBeInTheDocument();
  expect(screen.getByLabelText("速度环 Kd")).toBeInTheDocument();
  expect(screen.queryByText(/设备清单未提供可写目标参数/)).not.toBeInTheDocument();
  expect(screen.getByText("5/8 通道")).toBeInTheDocument();
  const focusedProfile = `schema_version: 1
vehicle: { id: focused-car, display_name: 聚焦车型, type: 测试, order: 50 }
control_loops:
  - id: speed
    label: 速度环
    gains: { Kp: pid.kp }
    recommended_channels: [drive.speed_mps]
`;
  act(() => {
    expect(useVehicleProfileStore.getState().importProfile(focusedProfile, false).status).toBe("imported");
    useVehicleProfileStore.getState().selectProfile("focused-car");
  });
  await waitFor(() => expect(screen.getByText("1/8 通道")).toBeInTheDocument());
  fireEvent.click(screen.getByRole("button", { name: "全部参数" }));
  expect(screen.getByRole("heading", { name: "参数目录" })).toBeInTheDocument();
});

it("falls back to the generic workspace for an empty incompatible profile", async () => {
  useVehicleProfileStore.getState().importProfile("schema_version: 1\nvehicle: { id: empty-car, display_name: 空车型, type: 测试, order: 50 }\n", false);
  useVehicleProfileStore.getState().selectProfile("empty-car");
  window.history.pushState({}, "", "/live/car-01");
  render(<AppProviders bridge={new MockBridge()}><App /></AppProviders>);
  await act(async () => undefined);
  expect(screen.getAllByText(/通用 Manifest/).length).toBeGreaterThan(0);
  expect(screen.getByRole("button", { name: "全部参数" })).toBeInTheDocument();
});
