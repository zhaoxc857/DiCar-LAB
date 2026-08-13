import { Robot } from "@phosphor-icons/react";
import { useRef, useState } from "react";
import { useDesktopBridge } from "../../app/providers";
import { DeepSeekClient } from "../../ai/aiClient";
import type { ParameterSnapshot } from "../../domain/types";
import { useCollaborationStore } from "../../stores/collaborationStore";
import { useConnectionStore } from "../../stores/connectionStore";
import { useSettingsStore } from "../../stores/settingsStore";
import { useTuningSnapshotStore } from "../../stores/tuningSnapshotStore";
import { useVehicleProfileStore } from "../../stores/vehicleProfileStore";
import { useWorkspaceStore } from "../../stores/workspaceStore";
import { runAutoTune, type AutoTuneResult, type RoundRecord, type TunableParam } from "../../tuning/autoTune";
import { captureTuningSnapshot } from "../../tuning/snapshots";
import { extractStepMetrics, type StepMetrics } from "../../tuning/metrics";
import type { ResolvedVehicleWorkspace } from "../../vehicleProfiles/types";
import { Button } from "../ui/button";
import { Input } from "../ui/input";
import { Label } from "../ui/label";
import { Select } from "../ui/select";

const SETTLE_MS = 800;
const BASELINE_WINDOW_US = 400_000;
const EXPERIMENT_SAMPLE_RATE_HZ = 100;
const DEFAULT_MAX_STEP_RATIO = 0.2;
const WATCHDOG = { maxOvershootPct: 80, maxOscillations: 6 };

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

export function AutoTuneWizard({
  open,
  onClose,
  workspace,
  records,
}: {
  open: boolean;
  onClose: () => void;
  workspace: ResolvedVehicleWorkspace;
  records: ParameterSnapshot[];
}) {
  const bridge = useDesktopBridge();
  const snapshot = useConnectionStore((state) => state.snapshot);
  const profile = useCollaborationStore((state) => state.profile);
  const profileId = useVehicleProfileStore((state) => state.selectedProfileId);
  const settings = useSettingsStore();
  const saveTuningSnapshot = useTuningSnapshotStore((state) => state.saveSnapshot);

  const eligibleLoops = workspace.controlLoops.filter(
    (loop) => loop.targetParamId !== null && loop.targetWritable && loop.telemetry.feedback !== null,
  );
  const [loopId, setLoopId] = useState<string>(eligibleLoops[0]?.id ?? "");
  const loop = eligibleLoops.find(({ id }) => id === loopId) ?? eligibleLoops[0];
  const gainRecords = (loop?.gainParamIds ?? [])
    .map(({ paramId }) => records.find((record) => record.paramId === paramId))
    .filter(
      (record): record is ParameterSnapshot =>
        record !== undefined && record.writable && !record.dangerous && record.numeric !== undefined &&
        (record.ramValue.kind === "f32" || record.ramValue.kind === "i32" || record.ramValue.kind === "u32"),
    );
  const [selectedParamIds, setSelectedParamIds] = useState<number[]>([]);
  const [apiKey, setApiKey] = useState(settings.aiApiKey);
  const [baseUrl, setBaseUrl] = useState(settings.aiBaseUrl);
  const [model, setModel] = useState(settings.aiModel);
  const [goal, setGoal] = useState("阶跃响应超调小于 5%，上升时间尽量短，无持续振荡");
  const [maxRounds, setMaxRounds] = useState(8);
  const [restValue, setRestValue] = useState(0);
  const [stepValue, setStepValue] = useState(1);
  const [holdMs, setHoldMs] = useState(3000);
  const [phase, setPhase] = useState<"config" | "running" | "done">("config");
  const [log, setLog] = useState<RoundRecord[]>([]);
  const [statusText, setStatusText] = useState("");
  const [result, setResult] = useState<AutoTuneResult | null>(null);
  const [saveMessage, setSaveMessage] = useState<string | null>(null);
  const abortRef = useRef(false);
  if (!open) return null;

  const chosen = gainRecords.filter(({ paramId }) => selectedParamIds.includes(paramId));
  const denial =
    profile.role === "observer"
      ? "仅观察者不能运行自动调参"
      : !profile.leaseActive
        ? "当前车辆控制权未激活"
        : snapshot?.phase !== "ready"
          ? "连接设备并同步参数后才能自动调参"
          : eligibleLoops.length === 0
            ? "当前车型没有可自动调参的控制环：需要可写的数值目标参数和反馈遥测通道"
            : apiKey.trim().length === 0
              ? "请先填写 DeepSeek API Key（只保存在本机）"
              : chosen.length === 0
                ? "请至少勾选一个要整定的增益参数"
                : stepValue === restValue
                  ? "阶跃值必须不同于静息值"
                  : null;

  async function start() {
    if (denial !== null || loop === undefined || loop.targetParamId === null) return;
    settings.saveAiSettings(baseUrl, model, apiKey);
    abortRef.current = false;
    setLog([]);
    setResult(null);
    setSaveMessage(null);
    setPhase("running");

    const targetRecord = records.find(({ paramId }) => paramId === loop.targetParamId);
    const targetKind = targetRecord?.ramValue.kind === "f32" ? "f32" : targetRecord?.ramValue.kind === "i32" ? "i32" : "u32";
    const feedbackChannel = loop.telemetry.feedback as number;
    const subscriptionChannels = [...new Set([loop.telemetry.target, feedbackChannel].filter((id): id is number => id !== null))];

    setStatusText("正在订阅实验所需的遥测通道…");
    const subscribed = await bridge.setTelemetrySubscription({
      channelIds: subscriptionChannels,
      sampleRateHz: EXPERIMENT_SAMPLE_RATE_HZ,
    });
    if (subscribed.status !== "succeeded") {
      setStatusText(`订阅遥测失败：${subscribed.message}`);
      setPhase("done");
      setResult({ status: "failed", message: subscribed.message, rounds: [], bestValues: null, bestRound: null });
      return;
    }

    const writeParam = async (paramId: number, kind: "f32" | "i32" | "u32", value: number) => {
      const outcome = await bridge.writeParameter(paramId, { kind, value } as ParameterSnapshot["ramValue"]);
      return outcome.status === "succeeded" ? null : outcome.message;
    };

    const runExperiment = async (): Promise<StepMetrics | null> => {
      const buffer = useWorkspaceStore.getState().buffer;
      const restFailure = await writeParam(loop.targetParamId as number, targetKind, restValue);
      if (restFailure !== null) return null;
      await sleep(SETTLE_MS);
      const stepAtUs = buffer.latest(feedbackChannel)?.timestampUs ?? 0;
      const baselinePoints = buffer
        .snapshot(feedbackChannel, stepAtUs - BASELINE_WINDOW_US)
        .filter((point) => point.timestampUs <= stepAtUs);
      const baseline =
        baselinePoints.length > 0
          ? baselinePoints.reduce((sum, point) => sum + (point.value.value as number), 0) / baselinePoints.length
          : restValue;
      const stepFailure = await writeParam(loop.targetParamId as number, targetKind, stepValue);
      if (stepFailure !== null) return null;
      await sleep(holdMs);
      const windowPoints = useWorkspaceStore.getState().buffer.snapshot(feedbackChannel, stepAtUs);
      await writeParam(loop.targetParamId as number, targetKind, restValue);
      return extractStepMetrics({ stepAtUs, baseline, target: stepValue, feedback: windowPoints });
    };

    setStatusText("循环运行中：AI 只写 RAM，随时可中止；首轮建议架空车轮。");
    const params: TunableParam[] = chosen.map((record) => ({
      paramId: record.paramId,
      machineName: record.machineName,
      displayName: record.displayName,
      unit: record.unit,
      min: record.numeric?.min ?? 0,
      max: record.numeric?.max ?? 0,
      maxStepRatio: DEFAULT_MAX_STEP_RATIO,
      initialValue: record.ramValue.value as number,
      kind: record.ramValue.kind as "f32" | "i32" | "u32",
    }));
    const outcome = await runAutoTune(
      { goal, maxRounds, params, watchdog: WATCHDOG },
      {
        writeParam,
        runExperiment,
        ai: new DeepSeekClient({ baseUrl, apiKey, model }),
        onRound: (record) => setLog((previous) => [...previous, record]),
        isAborted: () => abortRef.current,
      },
    );
    setResult(outcome);
    setStatusText(outcome.message);
    setPhase("done");
  }

  async function saveResult() {
    const fresh = await bridge.getSnapshot();
    const captured = captureTuningSnapshot(fresh, {
      name: `AI 调参 ${new Date().toLocaleString("zh-CN")}`,
      note: `${goal}（${result?.status ?? ""}，${log.length} 轮）`,
      origin: "manual",
      profileId,
      nowMs: Date.now(),
      id: crypto.randomUUID(),
    });
    if (captured === null) {
      setSaveMessage("当前没有可保存的参数");
      return;
    }
    const saved = saveTuningSnapshot(captured);
    setSaveMessage(saved.status === "saved" ? "已保存为参数方案" : saved.message);
  }

  return (
    <div
      className="fixed inset-0 z-50 grid place-items-center bg-black/70 p-4"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget && phase !== "running") onClose();
      }}
    >
      <section
        aria-labelledby="autotune-title"
        aria-modal="true"
        className="max-h-[85vh] w-full max-w-3xl overflow-auto rounded-[var(--radius)] border border-(--border) bg-(--surface-raised) shadow-2xl"
        role="dialog"
      >
        <header className="border-b border-(--border) p-4">
          <h2 className="m-0 inline-flex items-center gap-2 text-base" id="autotune-title">
            <Robot size={18} />
            AI 自动调参
          </h2>
          <p className="m-0 mt-1 text-xs text-(--text-muted)">
            自动循环：阶跃实验 → 本地指标 → DeepSeek 决策 → 限幅写入 RAM。永不自动固化 Flash；看门狗失稳即回滚。
          </p>
        </header>

        {phase === "config" && (
          <div className="space-y-4 p-4">
            <section className="rounded-[var(--radius)] border border-(--border) p-3">
              <h3 className="m-0 text-sm">DeepSeek 连接</h3>
              <div className="mt-2 grid gap-2 sm:grid-cols-3">
                <div>
                  <Label htmlFor="ai-key">API Key（仅本机保存）</Label>
                  <Input id="ai-key" onChange={(event) => setApiKey(event.currentTarget.value)} type="password" value={apiKey} />
                </div>
                <div>
                  <Label htmlFor="ai-model">模型</Label>
                  <Input id="ai-model" onChange={(event) => setModel(event.currentTarget.value)} value={model} />
                </div>
                <div>
                  <Label htmlFor="ai-url">Base URL</Label>
                  <Input id="ai-url" onChange={(event) => setBaseUrl(event.currentTarget.value)} value={baseUrl} />
                </div>
              </div>
            </section>

            <section className="rounded-[var(--radius)] border border-(--border) p-3">
              <h3 className="m-0 text-sm">实验设置</h3>
              <div className="mt-2 grid gap-2 sm:grid-cols-2">
                <div>
                  <Label htmlFor="autotune-loop">控制环</Label>
                  <Select id="autotune-loop" onChange={(event) => setLoopId(event.currentTarget.value)} value={loop?.id ?? ""}>
                    {eligibleLoops.map((candidate) => (
                      <option key={candidate.id} value={candidate.id}>
                        {candidate.label}
                      </option>
                    ))}
                  </Select>
                </div>
                <div>
                  <Label htmlFor="autotune-rounds">最大轮数</Label>
                  <Input
                    id="autotune-rounds"
                    max={30}
                    min={2}
                    onChange={(event) => setMaxRounds(Math.max(2, Math.min(30, Number(event.currentTarget.value) || 2)))}
                    type="number"
                    value={maxRounds}
                  />
                </div>
                <div>
                  <Label htmlFor="autotune-rest">静息目标值</Label>
                  <Input id="autotune-rest" onChange={(event) => setRestValue(Number(event.currentTarget.value) || 0)} type="number" value={restValue} />
                </div>
                <div>
                  <Label htmlFor="autotune-step">阶跃目标值</Label>
                  <Input id="autotune-step" onChange={(event) => setStepValue(Number(event.currentTarget.value) || 0)} type="number" value={stepValue} />
                </div>
                <div>
                  <Label htmlFor="autotune-hold">每轮保持时长 (ms)</Label>
                  <Input
                    id="autotune-hold"
                    max={10_000}
                    min={1000}
                    onChange={(event) => setHoldMs(Math.max(1000, Math.min(10_000, Number(event.currentTarget.value) || 3000)))}
                    type="number"
                    value={holdMs}
                  />
                </div>
                <div>
                  <Label htmlFor="autotune-goal">调参目标</Label>
                  <Input id="autotune-goal" onChange={(event) => setGoal(event.currentTarget.value)} value={goal} />
                </div>
              </div>
            </section>

            <section className="rounded-[var(--radius)] border border-(--border) p-3">
              <h3 className="m-0 text-sm">要整定的增益（单轮变化 ≤ 量程的 {DEFAULT_MAX_STEP_RATIO * 100}%）</h3>
              {gainRecords.length === 0 ? (
                <p className="m-0 mt-2 text-xs text-(--text-muted)">当前控制环没有可整定的数值增益参数。</p>
              ) : (
                <div className="mt-2 flex flex-wrap gap-3">
                  {gainRecords.map((record) => (
                    <label className="inline-flex items-center gap-1.5 text-xs" key={record.paramId}>
                      <input
                        checked={selectedParamIds.includes(record.paramId)}
                        onChange={(event) => {
                          const checked = event.currentTarget.checked;
                          setSelectedParamIds((previous) =>
                            checked ? [...previous, record.paramId] : previous.filter((id) => id !== record.paramId),
                          );
                        }}
                        type="checkbox"
                      />
                      {record.displayName}
                      <span className="font-mono text-(--text-muted)">
                        [{record.numeric?.min}–{record.numeric?.max}]
                      </span>
                    </label>
                  ))}
                </div>
              )}
            </section>

            {denial !== null && <p className="m-0 text-xs text-(--warning)">{denial}</p>}
            <p className="m-0 text-xs text-(--warning)">安全提示：首轮实验请将车辆架空或置于安全场地，手边保留断电开关。</p>
          </div>
        )}

        {phase !== "config" && (
          <div className="space-y-3 p-4">
            <p aria-live="polite" className="m-0 text-xs text-(--text-muted)">
              {statusText}
            </p>
            <div className="overflow-x-auto">
              <table className="w-full border-collapse text-left text-xs">
                <thead>
                  <tr className="border-b border-(--border) text-(--text-muted)">
                    <th className="p-2">轮</th>
                    <th className="p-2">参数</th>
                    <th className="p-2">超调</th>
                    <th className="p-2">上升</th>
                    <th className="p-2">稳态误差</th>
                    <th className="p-2">振荡</th>
                    <th className="p-2">评分</th>
                    <th className="p-2">AI 说明</th>
                  </tr>
                </thead>
                <tbody>
                  {log.map((record) => (
                    <tr className="border-b border-(--border)" key={record.round}>
                      <td className="p-2 font-mono">{record.round}</td>
                      <td className="p-2 font-mono">
                        {Object.entries(record.values)
                          .map(([paramId, value]) => `${records.find((candidate) => candidate.paramId === Number(paramId))?.displayName ?? paramId}=${Number(value.toFixed(4))}`)
                          .join(", ")}
                      </td>
                      <td className="p-2 font-mono">{record.metrics.overshootPct?.toFixed(1) ?? "—"}%</td>
                      <td className="p-2 font-mono">{record.metrics.riseTimeMs?.toFixed(0) ?? "—"}ms</td>
                      <td className="p-2 font-mono">{record.metrics.steadyStateErrorPct?.toFixed(1) ?? "—"}%</td>
                      <td className="p-2 font-mono">{record.metrics.oscillationCount}</td>
                      <td className="p-2 font-mono">{record.score.toFixed(1)}</td>
                      <td className="p-2">{[record.aiReason, ...record.clamped].filter(Boolean).join("；") || "—"}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
            {result !== null && (
              <p className={`m-0 text-xs ${result.status === "converged" ? "text-(--interactive)" : result.status === "watchdog" || result.status === "failed" ? "text-(--warning)" : "text-(--text-muted)"}`}>
                {result.message}
                {result.bestRound !== null ? `（已保留第 ${result.bestRound} 轮参数在 RAM）` : ""}
              </p>
            )}
            {saveMessage !== null && <p className="m-0 text-xs text-(--text-muted)">{saveMessage}</p>}
          </div>
        )}

        <footer className="flex justify-end gap-2 border-t border-(--border) p-4">
          {phase === "config" && (
            <>
              <Button onClick={onClose} variant="secondary">取消</Button>
              <Button disabled={denial !== null} onClick={() => void start()}>开始自动调参</Button>
            </>
          )}
          {phase === "running" && (
            <Button onClick={() => { abortRef.current = true; setStatusText("正在中止：本轮结束后回滚最佳参数…"); }} variant="secondary">
              中止并回滚
            </Button>
          )}
          {phase === "done" && (
            <>
              <Button onClick={() => void saveResult()} variant="secondary">保存为参数方案</Button>
              <Button onClick={() => setPhase("config")} variant="secondary">再来一轮</Button>
              <Button onClick={onClose}>完成</Button>
            </>
          )}
        </footer>
      </section>
    </div>
  );
}
