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
  const metrics = [
    ["接收字节", diagnostics?.inboundBytes ?? 0], ["发送字节", diagnostics?.outboundBytes ?? 0],
    ["往返时延", `${diagnostics?.lastRttMs ?? 0} ms`], ["有效帧", diagnostics?.validFrames ?? 0],
    ["CRC 错误", diagnostics?.crcErrors ?? 0], ["解码溢出", diagnostics?.decoderOverflows ?? 0],
    ["序列缺口", diagnostics?.sequenceGapSamples ?? 0], ["设备丢样", diagnostics?.deviceDroppedSamples ?? 0],
    ["UI 丢批次", diagnostics?.uiDroppedBatches ?? 0], ["最后有效帧", `${diagnostics?.lastValidFrameAtMs ?? 0} ms`],
  ];
  return (
    <main className="mx-auto w-full max-w-7xl px-4 py-6 lg:px-6" id="main-content">
      <Link className="inline-flex items-center gap-1 text-xs text-(--interactive)" to="/"><ArrowLeft size={14} />返回工作区</Link>
      <div className="mt-4 flex flex-wrap items-end justify-between gap-3"><div><h1 className="m-0 text-2xl font-semibold">连接与链路诊断</h1><p className="mb-0 mt-2 text-sm text-(--text-muted)">直接读取 AppActor 的权威快照，不估算设备状态。</p></div><span className={ready ? "inline-flex items-center gap-2 text-sm text-(--success)" : "inline-flex items-center gap-2 text-sm text-(--warning)"}>{ready ? <CheckCircle size={18} /> : <WarningCircle size={18} />}{connectionLabel(snapshot)}</span></div>
      <section className="mt-5 grid gap-3 lg:grid-cols-3" aria-label="设备身份">
        <Identity label="端点" value={endpoint} /><Identity label="会话 ID" value={session} /><Identity label="固件版本" value={firmware} />
        <Identity label="设备 ID" value={snapshot?.deviceIdHex ?? "—"} /><Identity label="SDK / 协商载荷" value="等待协议快照扩展" /><Identity label="断开原因" value={snapshot?.lastDisconnectReason ?? "无"} />
      </section>
      <section className="mt-4" aria-labelledby="link-metrics"><h2 className="mb-3 mt-0 text-base font-semibold" id="link-metrics">链路指标</h2><div className="grid gap-2 sm:grid-cols-2 lg:grid-cols-5">{metrics.map(([label, value]) => <Card className="p-3" key={label}><span className="block text-[11px] text-(--text-muted)">{label}</span><strong className="mt-1 block font-mono text-sm tabular-nums">{value}</strong></Card>)}</div></section>
    </main>
  );
}

function Identity({ label, value }: { label: string; value: string }) {
  return <Card className="p-4"><span className="text-[11px] uppercase tracking-wide text-(--text-muted)">{label}</span><strong className="mt-2 block break-all font-mono text-sm">{value}</strong></Card>;
}
