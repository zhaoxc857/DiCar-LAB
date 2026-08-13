import { MAX_SPEED_MPS, SpeedLoopModel, type SpeedLoopInput } from "./speedLoopModel";

const defaults = (targetMps: number): SpeedLoopInput => ({
  targetMps,
  kp: 1.2,
  ki: 0.08,
  kd: 0.002,
});

it("keeps a zero target stopped with finite output", () => {
  const model = new SpeedLoopModel();
  const input = defaults(0);

  model.advanceTo(3_000_000, input);
  const state = model.snapshot(input);

  expect(state.speedMps).toBe(0);
  expect(state.errorMps).toBe(0);
  expect(Number.isFinite(state.motorOutput)).toBe(true);
});

it("rises and reaches a stable finite response for the default step", () => {
  const model = new SpeedLoopModel();
  const input = defaults(1);

  model.advanceTo(500_000, input);
  const early = model.snapshot(input).speedMps;
  model.advanceTo(3_000_000, input);
  const late = model.snapshot(input).speedMps;

  expect(early).toBeGreaterThan(0.1);
  expect(late).toBeGreaterThan(early);
  expect(late).toBeGreaterThan(0.75);
  expect(late).toBeLessThanOrEqual(MAX_SPEED_MPS);
});

it("clears the integrator at zero between repeated steps", () => {
  const model = new SpeedLoopModel();
  const run = { targetMps: 1, kp: 0.6, ki: 0.5, kd: 0.01 };

  model.advanceTo(3_000_000, run);
  const stop = { ...run, targetMps: 0 };
  model.advanceTo(3_800_000, stop);
  expect(Math.abs(model.snapshot(stop).speedMps)).toBeLessThan(0.08);

  model.advanceTo(6_800_000, run);
  expect(model.snapshot(run).speedMps).toBeGreaterThan(0.7);
});

it("changes the three-second response when PID gains change", () => {
  const response = (input: SpeedLoopInput) => {
    const model = new SpeedLoopModel();
    model.advanceTo(3_000_000, input);
    return model.snapshot(input);
  };

  const low = response({ targetMps: 1, kp: 0.4, ki: 0.02, kd: 0 });
  const higher = response({ targetMps: 1, kp: 1.2, ki: 0.5, kd: 0.002 });

  expect(higher.speedMps).toBeGreaterThan(low.speedMps + 0.05);
  expect(Math.abs(higher.motorOutput)).toBeLessThanOrEqual(1);
});

it("sanitizes non-finite inputs and outputs", () => {
  const model = new SpeedLoopModel();
  const input = { targetMps: Number.NaN, kp: Number.POSITIVE_INFINITY, ki: 0.1, kd: 0 };

  model.advanceTo(10_000, input);
  const state = model.snapshot(input);

  expect(Number.isFinite(state.speedMps)).toBe(true);
  expect(Number.isFinite(state.errorMps)).toBe(true);
  expect(Number.isFinite(state.motorOutput)).toBe(true);
});
