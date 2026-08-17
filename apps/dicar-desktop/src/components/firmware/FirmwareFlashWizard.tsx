import { X } from "@phosphor-icons/react";
import * as Dialog from "@radix-ui/react-dialog";
import { useEffect, useRef, useState } from "react";
import { useFirmwareFlashPlatform } from "../../app/providers";
import type { FirmwareFlashPhase, FirmwarePackageSummary } from "../../firmware/firmwareTypes";
import { Alert } from "../ui/alert";
import { Button } from "../ui/button";

export type FirmwareFlashWizardPhase = "selecting" | "validating" | "ready" | "failed" | FirmwareFlashPhase;

type Props = {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  currentVersion: [number, number, number] | null;
  onPhaseChange?: (phase: FirmwareFlashWizardPhase, progressPercent?: number, message?: string) => void;
};

const CRITICAL_PHASES = new Set<FirmwareFlashWizardPhase>([
  "preparing",
  "switchingTransport",
  "unlocking",
  "erasing",
  "programming",
  "verifying",
  "restarting",
  "reconnecting",
  "retrying",
  "rollingBack",
]);

export function FirmwareFlashWizard({
  open,
  onOpenChange,
  currentVersion,
  onPhaseChange,
}: Props) {
  const platform = useFirmwareFlashPlatform();
  const [phase, setPhase] = useState<FirmwareFlashWizardPhase>("selecting");
  const phaseRef = useRef<FirmwareFlashWizardPhase>(phase);
  const [summary, setSummary] = useState<FirmwarePackageSummary | null>(null);
  const [packageBytes, setPackageBytes] = useState<Uint8Array | null>(null);
  const [confirmedStopped, setConfirmedStopped] = useState(false);
  const [allowDowngrade, setAllowDowngrade] = useState(false);
  const [operationId, setOperationId] = useState<string | null>(null);
  const [progressPercent, setProgressPercent] = useState(0);
  const [message, setMessage] = useState("");

  useEffect(() => {
    if (open || phase === "recoveryRequired") return;
    changePhase("selecting");
    setSummary(null);
    setPackageBytes(null);
    setConfirmedStopped(false);
    setAllowDowngrade(false);
    setOperationId(null);
    setProgressPercent(0);
    setMessage("");
  // Reset only when the containing dialog is closed outside a retained recovery.
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open]);

  const downgrade = summary !== null
    && currentVersion !== null
    && compareVersion(summary.firmwareVersion, currentVersion) < 0;
  const critical = CRITICAL_PHASES.has(phase);

  function changePhase(next: FirmwareFlashWizardPhase, progress?: number, nextMessage?: string) {
    phaseRef.current = next;
    setPhase(next);
    onPhaseChange?.(next, progress, nextMessage);
  }

  async function selectPackage(file: File | undefined) {
    if (file === undefined) return;
    changePhase("validating");
    setMessage("");
    setSummary(null);
    setConfirmedStopped(false);
    setAllowDowngrade(false);
    try {
      const bytes = new Uint8Array(await file.arrayBuffer());
      const inspected = await platform.inspect(bytes);
      setPackageBytes(bytes);
      setSummary(inspected);
      changePhase("ready");
    } catch (reason) {
      setPackageBytes(null);
      setMessage(errorMessage(reason, "固件包校验失败"));
      changePhase("failed");
    }
  }

  function handleEvent(event: {
    operationId: string;
    phase: FirmwareFlashPhase;
    progressPercent: number;
    message: string;
  }) {
    setOperationId(event.operationId);
    setProgressPercent(event.progressPercent);
    setMessage(event.message);
    changePhase(event.phase, event.progressPercent, event.message);
  }

  async function startFlash() {
    if (packageBytes === null || summary === null || !confirmedStopped) return;
    if (downgrade && !allowDowngrade) return;
    const nextOperationId = crypto.randomUUID();
    setOperationId(nextOperationId);
    setProgressPercent(0);
    setMessage("正在提交升级请求");
    changePhase("preparing", 0, "正在提交升级请求");
    try {
      await platform.start({
        operationId: nextOperationId,
        packageBytes,
        allowDowngrade,
      }, handleEvent);
      setProgressPercent(100);
      changePhase("succeeded", 100, "固件升级完成");
    } catch (reason) {
      if (phaseRef.current === "recoveryRequired") return;
      setMessage(errorMessage(reason, "固件升级失败"));
      changePhase("failed");
    }
  }

  async function resume(kind: "retry" | "rollback") {
    if (operationId === null) return;
    changePhase(kind === "retry" ? "retrying" : "rollingBack", 0);
    setMessage(kind === "retry" ? "正在重试候选固件" : "正在刷回恢复包");
    try {
      if (kind === "retry") {
        await platform.retry(operationId, handleEvent);
      } else {
        await platform.rollback(operationId, handleEvent);
      }
      setProgressPercent(100);
      changePhase("succeeded", 100, kind === "retry" ? "固件重试完成" : "恢复包回滚完成");
    } catch (reason) {
      setMessage(errorMessage(reason, "恢复操作失败"));
      changePhase("recoveryRequired", 0);
    }
  }

  function requestOpenChange(next: boolean) {
    if (!next && critical) return;
    onOpenChange(next);
  }

  return (
    <Dialog.Root onOpenChange={requestOpenChange} open={open}>
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 z-60 bg-black/70" />
        <Dialog.Content className="fixed left-1/2 top-1/2 z-70 max-h-[90vh] w-[min(94vw,680px)] -translate-x-1/2 -translate-y-1/2 overflow-auto rounded-[var(--radius)] border border-(--border) bg-(--surface-raised) shadow-2xl">
          <header className="flex items-start gap-3 border-b border-(--border) p-4">
            <div className="min-w-0 flex-1">
              <Dialog.Title className="m-0 text-base font-semibold">无线固件升级</Dialog.Title>
              <Dialog.Description className="m-0 mt-1 text-xs leading-5 text-(--text-muted)">
                天猛星 MSPM0G3507 · HC-05 / nanoUART-wl · TI ROM BSL 9600 8N1
              </Dialog.Description>
            </div>
            {!critical && (
              <Dialog.Close asChild>
                <button aria-label="关闭固件升级" className="grid size-10 place-items-center rounded-[var(--radius-sm)] text-(--text-muted)" type="button">
                  <X aria-hidden="true" size={18} />
                </button>
              </Dialog.Close>
            )}
          </header>

          <div className="space-y-4 p-4">
            {(phase === "selecting" || phase === "validating" || phase === "failed" || phase === "ready") && (
              <>
                <label className="block text-sm font-medium">
                  选择 .dicarfw 固件包
                  <input
                    accept=".dicarfw"
                    className="mt-2 block w-full text-sm"
                    disabled={phase === "validating"}
                    onChange={(event) => void selectPackage(event.currentTarget.files?.[0])}
                    type="file"
                  />
                </label>
                {phase === "validating" && <p aria-live="polite">正在验证签名、目标和镜像摘要…</p>}
                {phase === "failed" && <Alert>{message}</Alert>}
                {summary !== null && (
                  <section aria-label="固件包摘要" className="rounded-[var(--radius)] border border-(--border) p-3 text-sm">
                    <h3 className="m-0 text-sm">目标版本 {summary.firmwareVersion.join(".")}</h3>
                    <p className="m-0 mt-2 text-xs text-(--text-muted)">{summary.mcu} · {summary.imageLength} 字节</p>
                    <p className="data-value m-0 mt-1 break-all text-[11px] text-(--text-muted)">SHA-256 {summary.imageSha256}</p>
                  </section>
                )}
                {summary !== null && downgrade && (
                  <label className="flex items-start gap-2 text-sm text-(--warning)">
                    <input
                      checked={allowDowngrade}
                      onChange={(event) => setAllowDowngrade(event.currentTarget.checked)}
                      type="checkbox"
                    />
                    目标版本低于当前版本，我确认执行降级
                  </label>
                )}
                {summary !== null && (
                  <label className="flex items-start gap-2 text-sm">
                    <input
                      checked={confirmedStopped}
                      onChange={(event) => setConfirmedStopped(event.currentTarget.checked)}
                      type="checkbox"
                    />
                    我确认已停止车辆、电机和高功率输出，并保持无线模块供电
                  </label>
                )}
                <div className="flex justify-end gap-2">
                  <Button onClick={() => requestOpenChange(false)} variant="secondary">取消</Button>
                  <Button
                    disabled={summary === null || !confirmedStopped || (downgrade && !allowDowngrade)}
                    onClick={() => void startFlash()}
                  >
                    开始无线烧录
                  </Button>
                </div>
              </>
            )}

            {critical && (
              <section aria-live="polite" className="space-y-3">
                <h3 className="m-0 text-base">固件升级进行中</h3>
                <p className="m-0 text-sm">{message}</p>
                <progress aria-label="固件升级进度" className="w-full" max={100} value={progressPercent} />
                <p className="m-0 text-xs text-(--warning)">此阶段请勿断开 HC-05、关闭应用或切断设备电源。</p>
              </section>
            )}

            {phase === "recoveryRequired" && (
              <section aria-live="assertive" className="space-y-3">
                <Alert>{message || "设备需要人工进入 TI ROM BSL"}</Alert>
                <h3 className="m-0 text-base">人工进入 BSL</h3>
                <ol className="m-0 space-y-2 pl-5 text-sm">
                  <li>保持 HC-05 连接并持续供电。</li>
                  <li>按住 BSL，按下并松开 RST。</li>
                  <li>松开 BSL，然后选择重试或刷回恢复包。</li>
                </ol>
                <div className="flex flex-wrap justify-end gap-2">
                  <Button onClick={() => void resume("retry")} variant="secondary">重试候选固件</Button>
                  <Button onClick={() => void resume("rollback")} variant="danger">刷回恢复包</Button>
                </div>
              </section>
            )}

            {phase === "succeeded" && (
              <section aria-live="polite" className="space-y-3">
                <h3 className="m-0 text-base text-(--success)">固件升级完成</h3>
                <p className="m-0 text-sm">设备身份与目标版本已在 DCTP 重连后核对。</p>
                <div className="flex justify-end"><Button onClick={() => requestOpenChange(false)}>完成</Button></div>
              </section>
            )}
          </div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}

function compareVersion(left: [number, number, number], right: [number, number, number]): number {
  for (let index = 0; index < 3; index += 1) {
    const difference = left[index] - right[index];
    if (difference !== 0) return difference;
  }
  return 0;
}

function errorMessage(reason: unknown, fallback: string): string {
  if (reason instanceof Error) return reason.message;
  if (
    typeof reason === "object"
    && reason !== null
    && "message" in reason
    && typeof reason.message === "string"
  ) {
    return reason.message;
  }
  return fallback;
}
