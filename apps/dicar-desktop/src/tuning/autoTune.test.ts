import type { AiChatClient } from "../ai/aiClient";
import { clampProposal, parseProposal, runAutoTune, type AutoTuneConfig, type RoundRecord } from "./autoTune";
import type { StepMetrics } from "./metrics";

const KP = 1;
const KI = 2;

function config(overrides: Partial<AutoTuneConfig> = {}): AutoTuneConfig {
  return {
    goal: "超调小于 5%",
    maxRounds: 6,
    params: [
      { paramId: KP, machineName: "pid.kp", displayName: "Kp", unit: "", min: 0, max: 10, maxStepRatio: 0.2, initialValue: 1, kind: "f32" },
      { paramId: KI, machineName: "pid.ki", displayName: "Ki", unit: "", min: 0, max: 100, maxStepRatio: 0.1, initialValue: 10, kind: "u32" },
    ],
    watchdog: { maxOvershootPct: 80, maxOscillations: 6 },
    ...overrides,
  };
}

function metrics(overrides: Partial<StepMetrics> = {}): StepMetrics {
  return {
    sampleCount: 50,
    riseTimeMs: 100,
    overshootPct: 10,
    settlingTimeMs: 300,
    steadyStateErrorPct: 2,
    oscillationCount: 1,
    feedbackMin: 0,
    feedbackMax: 2.2,
    ...overrides,
  };
}

function scriptedAi(responses: string[]): AiChatClient {
  let index = 0;
  return {
    complete: () => Promise.resolve(responses[Math.min(index++, responses.length - 1)]),
  };
}

interface Harness {
  writes: Array<{ paramId: number; value: number }>;
  rounds: RoundRecord[];
  aborted: boolean;
  controller?: AbortController;
}

function deps(harness: Harness, experiments: Array<StepMetrics | null>, ai: AiChatClient) {
  let round = 0;
  return {
    writeParam: (paramId: number, _kind: "f32" | "i32" | "u32", value: number) => {
      harness.writes.push({ paramId, value });
      return Promise.resolve(null);
    },
    runExperiment: () => Promise.resolve(experiments[Math.min(round++, experiments.length - 1)]),
    ai,
    onRound: (record: RoundRecord) => {
      harness.rounds.push(record);
    },
    isAborted: () => harness.aborted,
    signal: harness.controller?.signal ?? new AbortController().signal,
  };
}

it("runs rounds until the AI declares convergence and keeps the best round on the device", async () => {
  const harness: Harness = { writes: [], rounds: [], aborted: false };
  const result = await runAutoTune(
    config(),
    deps(
      harness,
      [metrics({ overshootPct: 30 }), metrics({ overshootPct: 3, riseTimeMs: 80 })],
      scriptedAi([
        '{"converged": false, "reason": "降低超调", "next": [{"paramId": 1, "value": 1.5}]}',
        '{"converged": true, "reason": "指标达标", "next": []}',
      ]),
    ),
  );
  expect(result.status).toBe("converged");
  expect(result.rounds).toHaveLength(2);
  expect(result.rounds[1].aiReason).toBe("降低超调");
  expect(result.bestRound).toBe(2);
  // 第 2 轮（Kp=1.5）分数更好，收敛后写回的最佳值就是它。
  expect(harness.writes.at(-2)).toEqual({ paramId: KP, value: 1.5 });
});

it("clamps AI proposals to bounds and per-round step limits", () => {
  const { values, clamped } = clampProposal(
    { converged: false, reason: "", next: [{ paramId: KP, value: 9 }, { paramId: KI, value: 14.6 }, { paramId: 99, value: 1 }] },
    { [KP]: 1, [KI]: 10 },
    config(),
  );
  expect(values[KP]).toBe(3); // 1 + 0.2*(10-0)
  expect(values[KI]).toBe(15); // u32 取整，步长 10 内
  expect(clamped.some((entry) => entry.includes("Kp"))).toBe(true);
  expect(clamped.some((entry) => entry.includes("白名单"))).toBe(true);
});

it("rolls back to the best round when the watchdog trips", async () => {
  const harness: Harness = { writes: [], rounds: [], aborted: false };
  const result = await runAutoTune(
    config(),
    deps(
      harness,
      [metrics({ overshootPct: 10 }), metrics({ overshootPct: 150, oscillationCount: 9 })],
      scriptedAi(['{"converged": false, "reason": "加大 Kp", "next": [{"paramId": 1, "value": 2.5}]}']),
    ),
  );
  expect(result.status).toBe("watchdog");
  expect(result.bestRound).toBe(1);
  // 回滚写回第 1 轮的初始 Kp=1。
  expect(harness.writes.at(-2)).toEqual({ paramId: KP, value: 1 });
});

it("stops at the round limit, on abort, and on malformed AI output", async () => {
  const limited: Harness = { writes: [], rounds: [], aborted: false };
  const limitResult = await runAutoTune(
    config({ maxRounds: 2 }),
    deps(limited, [metrics()], scriptedAi(['{"converged": false, "reason": "试", "next": [{"paramId": 1, "value": 1.2}]}'])),
  );
  expect(limitResult.status).toBe("roundLimit");
  expect(limitResult.rounds).toHaveLength(2);

  const aborting: Harness = { writes: [], rounds: [], aborted: true };
  const abortResult = await runAutoTune(config(), deps(aborting, [metrics()], scriptedAi(["{}"])));
  expect(abortResult.status).toBe("aborted");

  const failing: Harness = { writes: [], rounds: [], aborted: false };
  const failResult = await runAutoTune(config(), deps(failing, [metrics()], scriptedAi(["这不是 JSON"])));
  expect(failResult.status).toBe("failed");
});

it("fails when the experiment window is invalid", async () => {
  const harness: Harness = { writes: [], rounds: [], aborted: false };
  const result = await runAutoTune(config(), deps(harness, [null], scriptedAi(["{}"])));
  expect(result.status).toBe("failed");
  expect(result.rounds).toHaveLength(0);
});

it("classifies an abort raised during an invalid experiment as aborted", async () => {
  const harness: Harness = { writes: [], rounds: [], aborted: false };
  const dependencies = deps(harness, [null], scriptedAi(["{}"]));
  dependencies.runExperiment = async () => {
    harness.aborted = true;
    return null;
  };

  const result = await runAutoTune(config(), dependencies);

  expect(result.status).toBe("aborted");
});

it("passes the cancellation signal to the AI client", async () => {
  const controller = new AbortController();
  const harness: Harness = { writes: [], rounds: [], aborted: false, controller };
  const ai: AiChatClient = {
    complete: async (_messages, signal) => {
      expect(signal).toBe(controller.signal);
      controller.abort();
      harness.aborted = true;
      throw new DOMException("aborted", "AbortError");
    },
  };

  const result = await runAutoTune(config(), deps(harness, [metrics()], ai));

  expect(result.status).toBe("aborted");
});

it("rejects proposals that neither converge nor suggest values", () => {
  expect(() => parseProposal('{"converged": false, "reason": "无", "next": []}', config())).toThrow();
  expect(() => parseProposal('{"converged": true, "reason": "好", "next": []}', config())).not.toThrow();
});
