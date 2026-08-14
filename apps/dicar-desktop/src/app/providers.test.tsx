import { render, screen } from "@testing-library/react";
import { MockBridge } from "../bridge/mockBridge";
import { UnavailableAiPlatform } from "../ai/aiPlatform";
import { AppProviders, useAiPlatform, useDesktopBridge } from "./providers";

function BridgeConsumer() {
  const bridge = useDesktopBridge();
  return <output>{bridge.constructor.name}</output>;
}

function AiConsumer() {
  const ai = useAiPlatform();
  return <output>{ai.available ? "available" : "unavailable"}</output>;
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

it("provides an independently injected AI platform", () => {
  const aiPlatform = new UnavailableAiPlatform();
  render(
    <AppProviders aiPlatform={aiPlatform} bridge={new MockBridge()}>
      <AiConsumer />
    </AppProviders>,
  );

  expect(screen.getByText("unavailable")).toBeInTheDocument();
});

it("fails clearly when a bridge consumer is rendered outside AppProviders", () => {
  expect(() => render(<BridgeConsumer />)).toThrow(
    "useDesktopBridge 必须在 AppProviders 内使用",
  );
});

it("fails clearly when an AI consumer is rendered outside AppProviders", () => {
  expect(() => render(<AiConsumer />)).toThrow(
    "useAiPlatform 必须在 AppProviders 内使用",
  );
});
