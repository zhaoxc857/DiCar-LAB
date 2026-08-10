import { render, screen } from "@testing-library/react";
import { App } from "./App";

it("shows the disconnected application shell and four menu destinations", () => {
  render(<App />);

  expect(screen.getByText("未连接")).toBeInTheDocument();
  expect(screen.getByRole("link", { name: /实时调参与波形/ })).toBeInTheDocument();
  expect(screen.getByRole("link", { name: /数据记录与回放/ })).toBeInTheDocument();
  expect(screen.getByRole("link", { name: /参数方案库/ })).toBeInTheDocument();
  expect(screen.getByRole("link", { name: /连接与链路诊断/ })).toBeInTheDocument();
});
