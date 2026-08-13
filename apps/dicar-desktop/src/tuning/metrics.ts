import type { TelemetryPoint } from "../domain/types";

/** 一轮阶跃实验的控制指标。AI 只见这些数字，不见原始波形。 */
export interface StepMetrics {
  sampleCount: number;
  riseTimeMs: number | null;
  overshootPct: number | null;
  settlingTimeMs: number | null;
  steadyStateErrorPct: number | null;
  oscillationCount: number;
  feedbackMin: number;
  feedbackMax: number;
}

export interface StepWindow {
  /** 阶跃发生时刻（设备微秒时钟）。 */
  stepAtUs: number;
  /** 阶跃前的反馈基线。 */
  baseline: number;
  /** 阶跃目标值（与反馈通道同量纲）。 */
  target: number;
  /** 阶跃后的反馈通道采样，按时间升序。 */
  feedback: TelemetryPoint[];
}

const SETTLE_BAND = 0.05;
const TAIL_FRACTION = 0.2;
export const MIN_SAMPLES = 8;

function numeric(point: TelemetryPoint): number {
  return point.value.value as number;
}

/**
 * 从阶跃响应窗口提取上升时间、超调、整定时间、稳态误差和振荡次数。
 * 阶跃幅度接近零或样本不足时返回 null（实验无效，不能喂给 AI）。
 */
export function extractStepMetrics(window: StepWindow): StepMetrics | null {
  const delta = window.target - window.baseline;
  const points = window.feedback.filter((point) => point.timestampUs >= window.stepAtUs);
  if (points.length < MIN_SAMPLES || delta === 0 || !Number.isFinite(delta)) return null;

  const direction = Math.sign(delta);
  const values = points.map(numeric);
  const feedbackMin = Math.min(...values);
  const feedbackMax = Math.max(...values);

  const lowThreshold = window.baseline + 0.1 * delta;
  const highThreshold = window.baseline + 0.9 * delta;
  let lowAtUs: number | null = null;
  let highAtUs: number | null = null;
  for (const point of points) {
    const value = numeric(point);
    if (lowAtUs === null && (value - lowThreshold) * direction >= 0) lowAtUs = point.timestampUs;
    if (highAtUs === null && (value - highThreshold) * direction >= 0) highAtUs = point.timestampUs;
    if (highAtUs !== null) break;
  }
  const riseTimeMs = lowAtUs !== null && highAtUs !== null ? (highAtUs - lowAtUs) / 1000 : null;

  const peak = direction > 0 ? feedbackMax : feedbackMin;
  const overshoot = ((peak - window.target) * direction) / Math.abs(delta);
  const overshootPct = overshoot > 0 ? overshoot * 100 : 0;

  const band = Math.abs(delta) * SETTLE_BAND;
  let settledAtUs: number | null = null;
  for (const point of points) {
    if (Math.abs(numeric(point) - window.target) <= band) {
      settledAtUs ??= point.timestampUs;
    } else {
      settledAtUs = null;
    }
  }
  const settlingTimeMs = settledAtUs !== null ? (settledAtUs - window.stepAtUs) / 1000 : null;

  const tail = values.slice(Math.max(1, Math.floor(values.length * (1 - TAIL_FRACTION))));
  const tailMean = tail.reduce((sum, value) => sum + value, 0) / tail.length;
  const steadyStateErrorPct = (Math.abs(tailMean - window.target) / Math.abs(delta)) * 100;

  let oscillationCount = 0;
  let previousSide = 0;
  for (const value of values) {
    const offset = value - window.target;
    if (Math.abs(offset) <= band) continue;
    const side = Math.sign(offset);
    if (previousSide !== 0 && side !== previousSide) oscillationCount += 1;
    previousSide = side;
  }

  return {
    sampleCount: points.length,
    riseTimeMs,
    overshootPct: riseTimeMs === null && overshootPct === 0 ? null : overshootPct,
    settlingTimeMs,
    steadyStateErrorPct,
    oscillationCount,
    feedbackMin,
    feedbackMax,
  };
}

/** 本地质量评分（越小越好）：用于选出最佳轮次，不信任 AI 的自评。 */
export function scoreMetrics(metrics: StepMetrics): number {
  const rise = metrics.riseTimeMs ?? 5000;
  const settle = metrics.settlingTimeMs ?? 5000;
  const overshoot = metrics.overshootPct ?? 100;
  const sse = metrics.steadyStateErrorPct ?? 100;
  return overshoot * 2 + sse * 3 + rise / 20 + settle / 40 + metrics.oscillationCount * 15;
}
