import { extractJsonObject, type AiChatClient, type AiChatMessage } from "../ai/aiClient";
import { scoreMetrics, type StepMetrics } from "./metrics";

/** 允许进入自动调参循环的单个参数（白名单 + 硬界）。 */
export interface TunableParam {
  paramId: number;
  machineName: string;
  displayName: string;
  unit: string;
  min: number;
  max: number;
  /** 单轮最大变化量占 (max-min) 的比例；越界建议会被本地截断。 */
  maxStepRatio: number;
  initialValue: number;
  kind: "f32" | "i32" | "u32";
}

export interface WatchdogLimits {
  maxOvershootPct: number;
  maxOscillations: number;
}

export interface AutoTuneConfig {
  goal: string;
  maxRounds: number;
  params: TunableParam[];
  watchdog: WatchdogLimits;
}

export interface RoundRecord {
  round: number;
  values: Record<number, number>;
  metrics: StepMetrics;
  score: number;
  aiReason: string | null;
  clamped: string[];
}

export type AutoTuneStatus = "converged" | "roundLimit" | "watchdog" | "aborted" | "failed";

export interface AutoTuneResult {
  status: AutoTuneStatus;
  message: string;
  rounds: RoundRecord[];
  /** 结束时已写回设备 RAM 的最佳参数值。 */
  bestValues: Record<number, number> | null;
  bestRound: number | null;
}

export interface AutoTuneDeps {
  /** 写入一个循环参数的 RAM 值，失败返回错误消息。 */
  writeParam(paramId: number, kind: TunableParam["kind"], value: number): Promise<string | null>;
  /** 执行一轮阶跃实验并返回指标；实验无效返回 null。 */
  runExperiment(): Promise<StepMetrics | null>;
  ai: AiChatClient;
  onRound(record: RoundRecord): void;
  isAborted(): boolean;
}

interface AiProposal {
  converged: boolean;
  reason: string;
  next: Array<{ paramId: number; value: number }>;
}

export async function runAutoTune(config: AutoTuneConfig, deps: AutoTuneDeps): Promise<AutoTuneResult> {
  const rounds: RoundRecord[] = [];
  let current: Record<number, number> = Object.fromEntries(
    config.params.map((param) => [param.paramId, param.initialValue]),
  );
  let best: { round: number; score: number; values: Record<number, number> } | null = null;
  let pendingClamps: string[] = [];
  let aiReason: string | null = null;

  const finish = async (status: AutoTuneStatus, message: string): Promise<AutoTuneResult> => {
    if (best !== null) {
      const failure = await writeValues(config, deps, best.values);
      if (failure !== null) {
        return { status: "failed", message: `写回最佳参数失败：${failure}`, rounds, bestValues: null, bestRound: null };
      }
    }
    return { status, message, rounds, bestValues: best?.values ?? null, bestRound: best?.round ?? null };
  };

  for (let round = 1; round <= config.maxRounds; round += 1) {
    if (deps.isAborted()) return finish("aborted", "已手动中止，设备保持最佳参数");

    const metrics = await deps.runExperiment();
    if (metrics === null) {
      return finish("failed", `第 ${round} 轮实验无效（样本不足或阶跃幅度为零），已停止`);
    }
    const score = scoreMetrics(metrics);
    const record: RoundRecord = { round, values: { ...current }, metrics, score, aiReason, clamped: pendingClamps };
    rounds.push(record);
    deps.onRound(record);
    if (best === null || score < best.score) best = { round, score, values: { ...current } };

    // 看门狗基于本地指标，AI 无法绕过：一旦失稳立即回滚最佳参数并结束。
    if (
      (metrics.overshootPct ?? 0) > config.watchdog.maxOvershootPct ||
      metrics.oscillationCount > config.watchdog.maxOscillations
    ) {
      return finish("watchdog", `第 ${round} 轮触发安全看门狗（超调/振荡超限），已回滚最佳参数`);
    }
    if (deps.isAborted()) return finish("aborted", "已手动中止，设备保持最佳参数");
    if (round === config.maxRounds) break;

    let proposal: AiProposal;
    try {
      proposal = parseProposal(await deps.ai.complete(buildMessages(config, rounds)), config);
    } catch (error) {
      return finish("failed", `AI 决策失败：${error instanceof Error ? error.message : String(error)}`);
    }
    aiReason = proposal.reason;
    if (proposal.converged) {
      return finish("converged", `AI 判定已收敛：${proposal.reason}`);
    }

    const { values, clamped } = clampProposal(proposal, current, config);
    pendingClamps = clamped;
    const failure = await writeValues(config, deps, values);
    if (failure !== null) return finish("failed", `写入参数失败：${failure}`);
    current = values;
  }

  return finish("roundLimit", `已达 ${config.maxRounds} 轮上限，设备保持最佳参数`);
}

async function writeValues(
  config: AutoTuneConfig,
  deps: AutoTuneDeps,
  values: Record<number, number>,
): Promise<string | null> {
  for (const param of config.params) {
    const value = values[param.paramId];
    if (value === undefined) continue;
    const failure = await deps.writeParam(param.paramId, param.kind, value);
    if (failure !== null) return `${param.displayName}：${failure}`;
  }
  return null;
}

/** 本地硬约束：范围 clamp + 单轮步长限幅 + 整数取整；返回实际写入值与截断说明。 */
export function clampProposal(
  proposal: AiProposal,
  current: Record<number, number>,
  config: AutoTuneConfig,
): { values: Record<number, number>; clamped: string[] } {
  const values = { ...current };
  const clamped: string[] = [];
  for (const next of proposal.next) {
    const param = config.params.find(({ paramId }) => paramId === next.paramId);
    if (param === undefined) {
      clamped.push(`忽略了白名单外的参数 ${next.paramId}`);
      continue;
    }
    const span = param.max - param.min;
    const maxStep = span * param.maxStepRatio;
    const from = values[param.paramId] ?? param.initialValue;
    let value = Math.min(Math.max(next.value, param.min), param.max);
    if (Math.abs(value - from) > maxStep) {
      value = from + Math.sign(value - from) * maxStep;
      clamped.push(`${param.displayName} 步长截断为 ${formatNumber(value)}`);
    } else if (value !== next.value) {
      clamped.push(`${param.displayName} 已限制在允许范围内`);
    }
    if (param.kind !== "f32") value = Math.round(value);
    values[param.paramId] = value;
  }
  return { values, clamped };
}

export function buildMessages(config: AutoTuneConfig, rounds: RoundRecord[]): AiChatMessage[] {
  const paramLines = config.params
    .map(
      (param) =>
        `- id=${param.paramId} ${param.machineName}（${param.displayName}）范围 [${formatNumber(param.min)}, ${formatNumber(param.max)}]${param.unit ? ` 单位 ${param.unit}` : ""}`,
    )
    .join("\n");
  const historyLines = rounds
    .map((record) => {
      const values = config.params
        .map((param) => `${param.machineName}=${formatNumber(record.values[param.paramId] ?? Number.NaN)}`)
        .join(", ");
      const metrics = record.metrics;
      return `第${record.round}轮 | ${values} | 上升 ${formatNumber(metrics.riseTimeMs)}ms, 超调 ${formatNumber(metrics.overshootPct)}%, 整定 ${formatNumber(metrics.settlingTimeMs)}ms, 稳态误差 ${formatNumber(metrics.steadyStateErrorPct)}%, 振荡 ${metrics.oscillationCount} 次`;
    })
    .join("\n");
  return [
    {
      role: "system",
      content:
        "你是嵌入式车辆控制环的调参助手。根据实验历史为下一轮提出参数值，或在指标已满足目标时判定收敛。" +
        '只输出一个 JSON 对象：{"converged": boolean, "reason": "中文一句话", "next": [{"paramId": number, "value": number}]}。' +
        "converged 为 true 时 next 为空数组。value 必须落在参数范围内，一次只做小幅调整。",
    },
    {
      role: "user",
      content: `调参目标：${config.goal}\n\n可调参数：\n${paramLines}\n\n实验历史（时间升序）：\n${historyLines}\n\n请给出下一轮参数或判定收敛。`,
    },
  ];
}

export function parseProposal(text: string, config: AutoTuneConfig): AiProposal {
  const raw = extractJsonObject(text) as Partial<AiProposal>;
  if (typeof raw.converged !== "boolean" || typeof raw.reason !== "string" || !Array.isArray(raw.next)) {
    throw new Error("AI 回复缺少 converged/reason/next 字段");
  }
  const next: AiProposal["next"] = [];
  for (const entry of raw.next) {
    const candidate = entry as { paramId?: unknown; value?: unknown };
    if (typeof candidate.paramId !== "number" || typeof candidate.value !== "number" || !Number.isFinite(candidate.value)) {
      throw new Error("AI 建议包含非法参数条目");
    }
    next.push({ paramId: candidate.paramId, value: candidate.value });
  }
  if (!raw.converged && next.length === 0) throw new Error("AI 未收敛却没有给出任何参数建议");
  void config;
  return { converged: raw.converged, reason: raw.reason, next };
}

function formatNumber(value: number | null): string {
  if (value === null || Number.isNaN(value)) return "—";
  return Number.isInteger(value) ? String(value) : value.toFixed(3).replace(/0+$/, "").replace(/\.$/, "");
}
