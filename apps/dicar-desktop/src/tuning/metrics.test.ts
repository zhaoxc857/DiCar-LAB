import type { TelemetryPoint } from "../domain/types";
import { extractStepMetrics, scoreMetrics } from "./metrics";

function points(values: number[], dtUs = 10_000, startUs = 0): TelemetryPoint[] {
  return values.map((value, index) => ({
    channelId: 200,
    timestampUs: startUs + index * dtUs,
    sampleSequence: index,
    value: { kind: "f32", value },
  }));
}

const window = (feedback: TelemetryPoint[]) => ({ stepAtUs: 0, baseline: 0, target: 2, feedback });

it("measures rise time, overshoot, settling, and steady-state error of an underdamped step", () => {
  const metrics = extractStepMetrics(
    window(points([0, 0.1, 0.5, 1.0, 1.5, 1.9, 2.4, 2.3, 2.1, 1.95, 2.02, 2.0, 2.0, 2.0, 2.0, 2.0])),
  );
  expect(metrics).not.toBeNull();
  expect(metrics?.riseTimeMs).toBe(30); // 0.5(10ms 达到 0.2 之后)…实际 0.2 阈值在 index2、1.8 阈值在 index5
  expect(metrics?.overshootPct).toBeCloseTo(20, 5);
  expect(metrics?.settlingTimeMs).toBe(90);
  expect(metrics?.steadyStateErrorPct).toBeLessThan(2);
  expect(metrics?.oscillationCount).toBeGreaterThanOrEqual(1);
  expect(metrics?.sampleCount).toBe(16);
});

it("rejects windows with too few samples or a zero step", () => {
  expect(extractStepMetrics(window(points([0, 1, 2])))).toBeNull();
  expect(
    extractStepMetrics({ stepAtUs: 0, baseline: 2, target: 2, feedback: points([2, 2, 2, 2, 2, 2, 2, 2, 2]) }),
  ).toBeNull();
});

it("handles negative steps with direction-aware thresholds", () => {
  const metrics = extractStepMetrics({
    stepAtUs: 0,
    baseline: 2,
    target: 0,
    feedback: points([2, 1.6, 1.0, 0.4, -0.15, 0.05, 0.0, 0.0, 0.0, 0.0]),
  });
  expect(metrics).not.toBeNull();
  expect(metrics?.overshootPct).toBeCloseTo(7.5, 5);
  expect(metrics?.riseTimeMs).not.toBeNull();
});

it("scores lower for better responses", () => {
  const sluggish = extractStepMetrics(window(points([0, 0.1, 0.2, 0.35, 0.5, 0.7, 0.9, 1.1, 1.25, 1.4, 1.5, 1.6])));
  const crisp = extractStepMetrics(window(points([0, 0.6, 1.4, 1.9, 2.02, 2.0, 2.0, 2.0, 2.0, 2.0, 2.0, 2.0])));
  expect(sluggish).not.toBeNull();
  expect(crisp).not.toBeNull();
  expect(scoreMetrics(crisp!)).toBeLessThan(scoreMetrics(sluggish!));
});
