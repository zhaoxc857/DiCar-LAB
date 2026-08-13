import { act, render, screen } from "@testing-library/react";
import { AppProviders } from "../../app/providers";
import { MockBridge } from "../../bridge/mockBridge";
import { TelemetryRingBuffer } from "../../telemetry/ringBuffer";
import { builtInProfiles } from "../../vehicleProfiles/catalog";
import { resolveVehicleWorkspace } from "../../vehicleProfiles/resolver";
import { ControlLoopWorkspace } from "./ControlLoopWorkspace";

it("renders a resolved speed loop with role cards and existing typed controls", async () => {
  const bridge = new MockBridge();
  const snapshot = await bridge.getSnapshot();
  const profile = builtInProfiles[0].profile;
  const loop = resolveVehicleWorkspace(profile, snapshot.parameters, snapshot.telemetryDescriptors).controlLoops[0];
  render(<AppProviders bridge={bridge}><ControlLoopWorkspace buffer={new TelemetryRingBuffer(8, 100)} descriptors={snapshot.telemetryDescriptors} loop={loop} records={snapshot.parameters} /></AppProviders>);
  await act(async () => undefined);
  expect(screen.getByText("目标", { exact: true })).toBeInTheDocument();
  expect(screen.getByText("实际", { exact: true })).toBeInTheDocument();
  expect(screen.getByText("误差", { exact: true })).toBeInTheDocument();
  expect(screen.getByLabelText("目标速度")).toBeInTheDocument();
  expect(screen.getByLabelText("速度环 Kp")).toBeInTheDocument();
  expect(screen.getByLabelText("速度环 Ki")).toBeInTheDocument();
  expect(screen.getByLabelText("速度环 Kd")).toBeInTheDocument();
  expect(screen.queryByText(/设备清单未提供可写目标参数/)).not.toBeInTheDocument();
});
