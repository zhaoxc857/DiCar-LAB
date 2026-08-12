import { formatTelemetry } from "./TelemetryLegend";

it("labels non-finite telemetry as an invalid sample", () => {
  expect(formatTelemetry(Number.NaN)).toBe("无效样本");
  expect(formatTelemetry(Number.POSITIVE_INFINITY)).toBe("无效样本");
});
