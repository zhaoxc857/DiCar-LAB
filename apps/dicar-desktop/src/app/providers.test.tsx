import { render, screen } from "@testing-library/react";
import { MockBridge } from "../bridge/mockBridge";
import { AppProviders, useDesktopBridge } from "./providers";

function BridgeConsumer() {
  const bridge = useDesktopBridge();
  return <output>{bridge.constructor.name}</output>;
}

it("provides one injected DesktopBridge instance to the React tree", () => {
  const bridge = new MockBridge();

  render(
    <AppProviders bridge={bridge}>
      <BridgeConsumer />
    </AppProviders>,
  );

  expect(screen.getByText("MockBridge")).toBeInTheDocument();
});

it("fails clearly when a bridge consumer is rendered outside AppProviders", () => {
  expect(() => render(<BridgeConsumer />)).toThrow(
    "useDesktopBridge 必须在 AppProviders 内使用",
  );
});
