import { act, render, screen } from "@testing-library/react";
import { MockBridge } from "../bridge/mockBridge";
import { App } from "./App";
import { AppProviders } from "./providers";

it("shows the disconnected application shell and four menu destinations", async () => {
  render(
    <AppProviders bridge={new MockBridge()}>
      <App />
    </AppProviders>,
  );
  await act(async () => undefined);

  expect(await screen.findByText("未连接")).toBeInTheDocument();
  expect(screen.getByRole("link", { name: /实时调参与波形/ })).toBeInTheDocument();
  expect(screen.getByRole("link", { name: /数据记录与回放/ })).toBeInTheDocument();
  expect(screen.getByRole("link", { name: /参数方案库/ })).toBeInTheDocument();
  expect(screen.getByRole("link", { name: /连接与链路诊断/ })).toBeInTheDocument();
});
