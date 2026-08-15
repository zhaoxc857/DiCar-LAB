import { ArrowLeft, CheckCircle, WarningCircle } from "@phosphor-icons/react";
import { Link } from "react-router";
import { Card } from "../components/ui/card";
import { endpointLabel } from "../domain/types";
import { connectionLabel, useConnectionStore } from "../stores/connectionStore";

export function DiagnosticsPage() {
  const snapshot = useConnectionStore((state) => state.snapshot);
  const ready = snapshot?.phase === "ready";
  const endpoint = endpointLabel(snapshot?.transportIdentity?.endpoint ?? null);
  const session = snapshot?.sessionId == null ? "—" : `0x${snapshot.sessionId.toString(16).padStart(8, "0")}`;
  const firmware = snapshot?.firmwareVersion?.join(".") ?? "—";
  const diagnostics = snapshot?.diagnostics;
  const budget = snapshot?.linkBudget;

  return (
    <main className="mx-auto w-full max-w-7xl px-4 py-6 lg:px-6" id="main-content">
      <Link className="inline-flex min-h-11 items-center gap-1 text-xs text-(--interactive)" to="/"><ArrowLeft aria-hidden="true" size={14} />返回工作区</Link>
      <header className="mt-3 flex flex-wrap items-end justify-between gap-3">
        <div>
          <h1 className="m-0 text-2xl font-semibold">连接与链路诊断</h1>
          <p className="mb-0 mt-2 text-sm text-(--text-muted)">直接来自设备与 AppActor 快照</p>
        </div>
        <span className={ready ? "inline-flex items-center gap-2 text-sm text-(--success)" : "inline-flex items-center gap-2 text-sm text-(--warning)"}>
          {ready ? <CheckCircle aria-hidden="true" size={18} /> : <WarningCircle aria-hidden="true" size={18} />}
          {connectionLabel(snapshot)}
        </span>
      </header>

      <section aria-labelledby="device-health" className="app-surface mt-5 p-4">
        <SectionHeader description="身份、固件与设备声明的链路预算" id="device-health" title="设备健康" />
        <div className="grid gap-2 sm:grid-cols-2 lg:grid-cols-3">
          <Identity label="端点" value={endpoint} />
          <Identity label="会话 ID" value={session} />
          <Identity label="固件版本" value={firmware} />
          <Identity label="设备 ID" value={snapshot?.deviceIdHex ?? "—"} />
          <Identity label="遥测安全上限" value={budget ? `${budget.maxChannels} 通道 × ${budget.maxSampleRateHz} Hz` : "—"} />
          <Identity label="断开原因" value={snapshot?.lastDisconnectReason ?? "无"} />
        </div>
        {budget && <p className="mb-0 mt-3 rounded-[var(--radius)] border border-(--border) bg-(--surface-raised) px-3 py-2 text-xs text-(--text-muted)">{budget.reason}</p>}
      </section>

      <section aria-labelledby="link-quality" className="app-surface mt-4 p-4">
        <SectionHeader description="订阅链路当前可直接观测的时延与丢失计数" id="link-quality" title="连接质量" />
        <div className="grid gap-2 sm:grid-cols-2 lg:grid-cols-5">
          <Metric label="往返时延" value={`${diagnostics?.lastRttMs ?? 0} ms`} />
          <Metric label="序列缺口" value={diagnostics?.sequenceGapSamples ?? 0} />
          <Metric label="设备丢样" value={diagnostics?.deviceDroppedSamples ?? 0} />
          <Metric label="UI 丢批次" value={diagnostics?.uiDroppedBatches ?? 0} />
          <Metric label="最后有效帧" value={`${diagnostics?.lastValidFrameAtMs ?? 0} ms`} />
        </div>
      </section>

      <section aria-labelledby="protocol-events" className="app-surface mt-4 p-4">
        <SectionHeader description="用于排查协议和解码问题的原始累计计数" id="protocol-events" title="协议事件" />
        <details className="rounded-[var(--radius)] border border-(--border) bg-(--surface)">
          <summary className="flex min-h-11 cursor-pointer items-center px-3 text-xs font-semibold text-(--text)">展开原始协议计数</summary>
          <div className="grid gap-2 border-t border-(--border) p-3 sm:grid-cols-2 lg:grid-cols-5">
            <Metric label="接收字节" value={diagnostics?.inboundBytes ?? 0} />
            <Metric label="发送字节" value={diagnostics?.outboundBytes ?? 0} />
            <Metric label="有效帧" value={diagnostics?.validFrames ?? 0} />
            <Metric label="畸形帧" value={diagnostics?.malformedFrames ?? 0} />
            <Metric label="CRC 错误" value={diagnostics?.crcErrors ?? 0} />
            <Metric label="解码溢出" value={diagnostics?.decoderOverflows ?? 0} />
            <Metric label="重试" value={diagnostics?.retries ?? 0} />
            <Metric label="丢弃未请求响应" value={diagnostics?.unsolicitedDropped ?? 0} />
            <Metric label="拒绝遥测批次" value={diagnostics?.rejectedTelemetryBatches ?? 0} />
          </div>
        </details>
      </section>
    </main>
  );
}

function SectionHeader({ description, id, title }: { description: string; id: string; title: string }) {
  return <div className="mb-3"><h2 className="m-0 text-base font-semibold" id={id}>{title}</h2><p className="mb-0 mt-1 text-xs text-(--text-muted)">{description}</p></div>;
}

function Identity({ label, value }: { label: string; value: string }) {
  return <Card className="p-3"><span className="text-[11px] text-(--text-muted)">{label}</span><strong className="data-value mt-1 block break-all text-sm">{value}</strong></Card>;
}

function Metric({ label, value }: { label: string; value: string | number }) {
  return <Card className="p-3"><span className="block text-[11px] text-(--text-muted)">{label}</span><strong className="data-value mt-1 block text-sm">{value}</strong></Card>;
}
