import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { App } from "../../app/App";
import { AppProviders } from "../../app/providers";
import { MockBridge } from "../../bridge/mockBridge";
import { useTuningSnapshotStore } from "../../stores/tuningSnapshotStore";
import { useVehicleProfileStore } from "../../stores/vehicleProfileStore";

beforeEach(() => {
  localStorage.clear();
  useTuningSnapshotStore.getState().reset();
  useVehicleProfileStore.getState().reset();
});

async function openWorkbench() {
  window.history.pushState({}, "", "/live/car-01");
  render(<AppProviders bridge={new MockBridge()}><App /></AppProviders>);
  await act(async () => undefined);
  fireEvent.click(screen.getByRole("button", { name: /打开设备连接/ }));
  fireEvent.click(screen.getByRole("button", { name: "连接模拟器" }));
  expect((await screen.findAllByText("已就绪")).length).toBeGreaterThan(0);
  fireEvent.click(screen.getByRole("button", { name: "关闭设备连接" }));
}

async function writeKp(value: string) {
  fireEvent.click(screen.getByRole("button", { name: /速度环 PID/ }));
  fireEvent.change(screen.getByLabelText("速度环 Kp"), { target: { value } });
  fireEvent.click(screen.getByRole("button", { name: "写入 RAM" }));
  await waitFor(() => expect(screen.getByLabelText("速度环 Kp")).toHaveValue(Number(value)));
}

it("saves a snapshot, then applies it to restore the previous RAM values", async () => {
  await openWorkbench();
  await writeKp("1.8");

  fireEvent.click(screen.getByRole("button", { name: "参数方案" }));
  const dialog = screen.getByRole("dialog", { name: "参数方案" });
  expect(dialog).toBeInTheDocument();
  fireEvent.change(screen.getByLabelText("方案名称"), { target: { value: "基准 1.8" } });
  fireEvent.click(screen.getByRole("button", { name: "保存方案" }));
  expect(await screen.findByText(/已保存「基准 1.8」/)).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "关闭" }));

  await writeKp("2.2");

  fireEvent.click(screen.getByRole("button", { name: "参数方案" }));
  fireEvent.click(screen.getByRole("button", { name: "应用" }));
  expect(screen.getByText(/1 项将写入 RAM/)).toBeInTheDocument();
  expect(screen.getByText("将写入 RAM")).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: /写入 1 项到 RAM/ }));
  expect(await screen.findByText(/已写入 1 项/)).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "关闭" }));

  await waitFor(() => expect(screen.getByLabelText("速度环 Kp")).toHaveValue(1.8));
});

it("records an automatic commit snapshot after flashing", async () => {
  await openWorkbench();
  await writeKp("1.6");
  await waitFor(() => expect(screen.getByText("1 项待固化")).toBeInTheDocument());

  fireEvent.click(screen.getByRole("button", { name: "审阅并固化" }));
  fireEvent.click(screen.getByRole("button", { name: "固化到 Flash" }));
  await waitFor(() => expect(screen.queryByText("0 项待固化")).not.toBeInTheDocument());

  fireEvent.click(screen.getByRole("button", { name: "参数方案" }));
  expect(await screen.findByText(/固化记录 · Gen \d+/)).toBeInTheDocument();
});

it("keeps observers read-only inside the snapshot manager", async () => {
  await openWorkbench();
  fireEvent.change(screen.getByLabelText("演示身份"), { target: { value: "observer" } });
  await screen.findByText("仅观察者不能修改参数");

  fireEvent.click(screen.getByRole("button", { name: "参数方案" }));
  expect(screen.getByText("仅观察者不能创建参数方案")).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "保存方案" })).toBeDisabled();
});
