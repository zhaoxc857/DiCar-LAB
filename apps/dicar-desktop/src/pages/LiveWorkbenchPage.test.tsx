import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { App } from "../app/App";
import { AppProviders } from "../app/providers";
import { MockBridge } from "../bridge/mockBridge";

it("runs the B-to-A tuning flow with permissions, RAM truth, and commit review", async () => {
  window.history.pushState({}, "", "/live/car-01");
  const bridge = new MockBridge();
  render(<AppProviders bridge={bridge}><App /></AppProviders>);
  await act(async () => undefined);

  expect(screen.getByRole("heading", { name: "实时调参与波形" })).toBeInTheDocument();
  expect(screen.getByRole("heading", { name: "参数目录" })).toBeInTheDocument();
  expect(screen.getByRole("heading", { name: "实时波形" })).toBeInTheDocument();
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
