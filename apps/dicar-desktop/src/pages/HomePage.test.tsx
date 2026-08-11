import { act, fireEvent, render, screen } from "@testing-library/react";
import { App } from "../app/App";
import { AppProviders } from "../app/providers";
import { MockBridge } from "../bridge/mockBridge";

it("renders the B-style menu and connects the real simulator destination", async () => {
  const bridge = new MockBridge();
  render(
    <AppProviders bridge={bridge}>
      <App />
    </AppProviders>,
  );
  await act(async () => undefined);

  expect(screen.getByRole("heading", { name: "工作区" })).toBeInTheDocument();
  expect(screen.getByRole("link", { name: /实时调参与波形/ })).toBeInTheDocument();
  expect(screen.getByRole("link", { name: /计划发布.*数据记录与回放/ })).toBeInTheDocument();
  expect(screen.getByRole("link", { name: /计划发布.*参数方案库/ })).toBeInTheDocument();
  expect(screen.getByRole("link", { name: /连接与链路诊断/ })).toBeInTheDocument();
  expect(screen.getByText("本地演示权限")).toBeInTheDocument();
  expect(screen.getByText("未连接")).toBeInTheDocument();

  fireEvent.click(screen.getByRole("button", { name: "连接模拟器" }));
  expect(await screen.findByText("已就绪")).toBeInTheDocument();
  expect(screen.getByText("16 个遥测通道")).toBeInTheDocument();
  expect(screen.getByText("1 个参数")).toBeInTheDocument();
});

it("keeps the skip link and exposes an honest deferred destination", async () => {
  render(
    <AppProviders bridge={new MockBridge()}>
      <App />
    </AppProviders>,
  );
  expect(screen.getByRole("link", { name: "跳至主要内容" })).toHaveAttribute(
    "href",
    "#main-content",
  );

  fireEvent.click(screen.getByRole("link", { name: /计划发布.*数据记录与回放/ }));
  expect(await screen.findByRole("heading", { name: "数据记录与回放" })).toBeInTheDocument();
  expect(screen.getByText(/首版后续阶段开放/)).toBeInTheDocument();
  expect(screen.getByRole("link", { name: "返回工作区" })).toHaveAttribute("href", "/");
});
