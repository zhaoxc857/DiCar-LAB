import { act, render, screen, within } from "@testing-library/react";
import { MockBridge } from "../bridge/mockBridge";
import { App } from "./App";
import { AppProviders } from "./providers";

it("shows the precision-console shell with four real global destinations", async () => {
  render(
    <AppProviders bridge={new MockBridge()}>
      <App />
    </AppProviders>,
  );
  await act(async () => undefined);

  expect(await screen.findByRole("button", { name: /未连接.*打开设备连接/ })).toBeInTheDocument();
  const navigation = screen.getByRole("navigation", { name: "主要导航" });
  expect(within(navigation).getByRole("link", { name: "概览" })).toBeInTheDocument();
  expect(within(navigation).getByRole("link", { name: "实时调试" })).toBeInTheDocument();
  expect(within(navigation).getByRole("link", { name: "波形记录" })).toBeInTheDocument();
  expect(within(navigation).getByRole("link", { name: "诊断" })).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "打开硬件帮助" })).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "打开设置" })).toBeInTheDocument();
});
