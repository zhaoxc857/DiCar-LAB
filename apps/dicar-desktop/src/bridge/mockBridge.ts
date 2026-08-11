import type {
  AppSnapshot,
  AccessProfileId,
  BridgeEvent,
  Endpoint,
  OperationResult,
  ParameterSnapshot,
  ParameterValue,
  TelemetryDescriptor,
  TelemetryPoint,
  TelemetrySubscriptionRequest,
  TelemetryValue,
  WindowCloseDecision,
} from "../domain/types";
import type { DesktopBridge } from "./desktopBridge";

type BridgeListener = (event: BridgeEvent) => void;
type UnindexedBridgeEvent<T = BridgeEvent> = T extends { eventIndex: number }
  ? Omit<T, "eventIndex">
  : never;

const telemetryDescriptors: TelemetryDescriptor[] = Array.from({ length: 16 }, (_, index) => ({
  channelId: 200 + index,
  telemetryType: index % 4 === 0 ? "f32" : index % 4 === 1 ? "i32" : index % 4 === 2 ? "u32" : "flags32",
  machineName: `mock.channel_${index}`,
  displayName: `模拟通道 ${index + 1}`,
  group: "模拟遥测",
  unit: index % 4 === 0 ? "m/s" : "raw",
}));

function disconnectedSnapshot(): AppSnapshot {
  return {
    revision: 0,
    phase: "disconnected",
    transportIdentity: null,
    sessionId: null,
    deviceIdHex: null,
    firmwareVersion: null,
    parameters: [pidParameter()],
    telemetryDescriptors,
    dirtyCount: 0,
    storageGeneration: 0,
    accessProfile: { role: "owner", leaseActive: true, localDemoOnly: true },
    desiredSubscription: null,
    activeSubscription: null,
    paused: false,
    telemetryPoints: 0,
    diagnostics: {
      inboundBytes: 0,
      outboundBytes: 0,
      lastRttMs: 0,
      lastValidFrameAtMs: 0,
      validFrames: 0,
      malformedFrames: 0,
      crcErrors: 0,
      decoderOverflows: 0,
      retries: 0,
      unsolicitedDropped: 0,
      sequenceGapSamples: 0,
      deviceDroppedSamples: 0,
      rejectedTelemetryBatches: 0,
      uiDroppedBatches: 0,
    },
    lastDisconnectReason: null,
    markers: [],
  };
}

function pidParameter(): ParameterSnapshot {
  return {
    paramId: 1,
    machineName: "pid.kp",
    displayName: "速度环 Kp",
    group: "速度环 PID",
    unit: "",
    ramValue: { kind: "f32", value: 1 },
    persistedValue: { kind: "f32", value: 1 },
    revision: 1,
    dirty: false,
    syncKnown: true,
    writeState: "idle",
    writable: true,
    dangerous: false,
    lastError: null,
  };
}

export class MockBridge implements DesktopBridge {
  readonly #listeners = new Set<BridgeListener>();
  #eventIndex = 0;
  #operationId = 0;
  #sampleSequence = 0;
  #timestampUs = 0;
  #subscriptionVersion = 1;
  #requestId = 0;
  #activeCloseRequest: number | null = null;
  #history: Array<{ paramId: number; value: ParameterValue }> = [];
  #snapshot = disconnectedSnapshot();

  async subscribe(listener: BridgeListener): Promise<() => void> {
    this.#listeners.add(listener);
    return () => this.#listeners.delete(listener);
  }

  async connect(endpoint: Endpoint): Promise<OperationResult> {
    this.#snapshot = {
      ...this.#snapshot,
      revision: this.#snapshot.revision + 1,
      phase: "ready",
      transportIdentity: { endpoint },
      sessionId: 0x44aa_0001,
      deviceIdHex: "00112233445566778899aabbccddeeff",
      firmwareVersion: [1, 0, 0],
      parameters: this.#snapshot.parameters.map((parameter) => ({
        ...parameter,
        syncKnown: true,
        writeState: "idle",
      })),
      activeSubscription: {
        subscriptionVersion: this.#subscriptionVersion,
        sampleRateHz: 500,
        channelIds: telemetryDescriptors.slice(0, 8).map(({ channelId }) => channelId),
      },
    };
    this.#snapshot.desiredSubscription = this.#snapshot.activeSubscription;
    this.#publish({ event: "snapshotChanged", data: this.#snapshot });
    return this.#complete("已连接模拟器");
  }

  async disconnect(): Promise<OperationResult> {
    this.#applyDisconnect("用户主动断开");
    return this.#complete("设备已断开");
  }

  async writeParameter(paramId: number, value: ParameterValue): Promise<OperationResult> {
    const denied = this.#writeDenial();
    if (denied !== null) return this.#fail(denied);
    const index = this.#snapshot.parameters.findIndex((parameter) => parameter.paramId === paramId);
    if (index < 0) return this.#fail("未知参数");
    const current = this.#snapshot.parameters[index];
    if (!current.writable) return this.#fail("参数只读");
    if (current.ramValue.kind !== value.kind) return this.#fail("参数类型不匹配");
    this.#history.push({ paramId, value: current.ramValue });
    const next: ParameterSnapshot = {
      ...current,
      ramValue: value,
      revision: (current.revision + 1) >>> 0,
      dirty: !sameValue(value, current.persistedValue),
      lastError: null,
    };
    this.#replaceParameter(index, next);
    return this.#complete("RAM 参数已确认");
  }

  async commitParameters(): Promise<OperationResult> {
    if (this.#snapshot.phase !== "ready") return this.#fail("设备未连接");
    if (this.#snapshot.accessProfile.role !== "owner") {
      return this.#fail("当前身份没有固化权限");
    }
    if (!this.#snapshot.accessProfile.leaseActive) return this.#fail("当前车辆控制权未激活");
    const dirty = this.#snapshot.parameters.some((parameter) => parameter.dirty);
    if (!dirty) return this.#complete("没有需要固化的参数");
    this.#snapshot = {
      ...this.#snapshot,
      revision: this.#snapshot.revision + 1,
      parameters: this.#snapshot.parameters.map((parameter) =>
        parameter.dirty
          ? { ...parameter, persistedValue: parameter.ramValue, dirty: false }
          : parameter,
      ),
      dirtyCount: 0,
      storageGeneration: (this.#snapshot.storageGeneration + 1) >>> 0,
    };
    this.#publishSnapshot();
    return this.#complete("参数已固化到 Flash");
  }

  async revertAll(): Promise<OperationResult> {
    const denied = this.#writeDenial();
    if (denied !== null) return this.#fail(denied);
    this.#snapshot = {
      ...this.#snapshot,
      revision: this.#snapshot.revision + 1,
      parameters: this.#snapshot.parameters.map((parameter) =>
        parameter.dirty && parameter.persistedValue !== null
          ? {
              ...parameter,
              ramValue: parameter.persistedValue,
              revision: (parameter.revision + 1) >>> 0,
              dirty: false,
            }
          : parameter,
      ),
      dirtyCount: 0,
    };
    this.#publishSnapshot();
    return this.#complete("全部未固化修改已回退");
  }

  async undoLast(): Promise<OperationResult> {
    const previous = this.#history.pop();
    if (previous === undefined) return this.#complete("没有可撤销的已确认修改");
    return this.writeParameter(previous.paramId, previous.value);
  }

  async setTelemetrySubscription(request: TelemetrySubscriptionRequest): Promise<OperationResult> {
    if (this.#snapshot.phase !== "ready") return this.#fail("设备未连接");
    const unique = new Set(request.channelIds);
    const known = new Set(this.#snapshot.telemetryDescriptors.map(({ channelId }) => channelId));
    if (
      request.channelIds.length === 0 ||
      request.channelIds.length > 8 ||
      unique.size !== request.channelIds.length ||
      request.channelIds.some((channelId) => !known.has(channelId)) ||
      request.sampleRateHz < 1 ||
      request.sampleRateHz > 500
    ) {
      return this.#fail("遥测订阅必须包含 1–8 个唯一已知通道，采样率为 1–500 Hz");
    }
    this.#subscriptionVersion = (this.#subscriptionVersion + 1) & 0xffff;
    if (this.#subscriptionVersion === 0) this.#subscriptionVersion = 1;
    const subscription = {
      ...request,
      channelIds: [...request.channelIds],
      subscriptionVersion: this.#subscriptionVersion,
    };
    this.#snapshot = {
      ...this.#snapshot,
      revision: this.#snapshot.revision + 1,
      desiredSubscription: subscription,
      activeSubscription: subscription,
      paused: false,
    };
    this.#publishSnapshot();
    return this.#complete("遥测订阅已生效");
  }

  async setPaused(paused: boolean): Promise<OperationResult> {
    if (this.#snapshot.phase !== "ready") return this.#fail("设备未连接");
    if (paused === this.#snapshot.paused) return this.#complete("波形状态未变化");
    if (paused) {
      this.#snapshot = {
        ...this.#snapshot,
        revision: this.#snapshot.revision + 1,
        paused: true,
        activeSubscription: null,
      };
    } else {
      const desired = this.#snapshot.desiredSubscription;
      if (desired === null) return this.#fail("尚未选择遥测通道");
      this.#subscriptionVersion = (this.#subscriptionVersion + 1) & 0xffff;
      if (this.#subscriptionVersion === 0) this.#subscriptionVersion = 1;
      const resumed = { ...desired, subscriptionVersion: this.#subscriptionVersion };
      this.#snapshot = {
        ...this.#snapshot,
        revision: this.#snapshot.revision + 1,
        paused: false,
        desiredSubscription: resumed,
        activeSubscription: resumed,
      };
    }
    this.#publishSnapshot();
    return this.#complete(paused ? "波形已暂停" : "波形已恢复");
  }

  async addMarker(label: string): Promise<OperationResult> {
    const bytes = new TextEncoder().encode(label).byteLength;
    if (bytes === 0 || bytes > 64) return this.#fail("标记文字必须为 1–64 字节");
    this.#snapshot = {
      ...this.#snapshot,
      revision: this.#snapshot.revision + 1,
      markers: [...this.#snapshot.markers.slice(-255), label],
    };
    this.#publishSnapshot();
    return this.#complete("波形标记已添加");
  }

  async selectAccessProfile(profile: AccessProfileId): Promise<OperationResult> {
    this.#snapshot = {
      ...this.#snapshot,
      revision: this.#snapshot.revision + 1,
      accessProfile: { role: profile, leaseActive: true, localDemoOnly: true },
    };
    this.#publishSnapshot();
    return this.#complete("本地演示权限已切换");
  }

  async getSnapshot(): Promise<AppSnapshot> {
    return structuredClone(this.#snapshot);
  }

  requestWindowClose(): number {
    if (this.#snapshot.dirtyCount === 0) return 0;
    if (this.#activeCloseRequest !== null) return this.#activeCloseRequest;
    const requestId = (this.#requestId += 1);
    this.#activeCloseRequest = requestId;
    this.#publish({
      event: "windowCloseRequested",
      data: {
        requestId,
        dirtyCount: this.#snapshot.dirtyCount,
        canRevert: this.#snapshot.phase === "ready",
      },
    });
    return requestId;
  }

  async resolveWindowClose(
    requestId: number,
    decision: WindowCloseDecision,
  ): Promise<OperationResult> {
    if (requestId === 0 || requestId !== this.#activeCloseRequest) {
      return this.#fail("关闭请求已失效");
    }
    this.#activeCloseRequest = null;
    if (decision === "cancel") return this.#complete("已取消关闭");
    if (decision === "revertThenClose") {
      const denied = this.#writeDenial();
      if (denied !== null) return this.#fail(denied);
      this.#snapshot = {
        ...this.#snapshot,
        parameters: this.#snapshot.parameters.map((parameter) =>
          parameter.dirty && parameter.persistedValue !== null
            ? { ...parameter, ramValue: parameter.persistedValue, dirty: false }
            : parameter,
        ),
        dirtyCount: 0,
      };
    }
    this.#applyDisconnect("窗口关闭");
    return this.#complete("窗口可安全关闭");
  }

  advanceTelemetry(sampleCount: number): void {
    const subscription = this.#snapshot.activeSubscription;
    if (subscription === null || sampleCount <= 0) return;
    const points: TelemetryPoint[] = [];
    for (let sample = 0; sample < sampleCount; sample += 1) {
      this.#timestampUs += 2_000;
      for (const [slot, channelId] of subscription.channelIds.entries()) {
        points.push({
          channelId,
          timestampUs: this.#timestampUs,
          sampleSequence: this.#sampleSequence & 0xffff,
          value: deterministicValue(slot, this.#sampleSequence),
        });
      }
      this.#sampleSequence += 1;
    }
    this.#publish({
      event: "telemetryBatch",
      data: {
        subscriptionVersion: subscription.subscriptionVersion,
        firstSampleSequence: (this.#sampleSequence - sampleCount) & 0xffff,
        droppedSamples: 0,
        points,
      },
    });
    this.#snapshot = {
      ...this.#snapshot,
      revision: this.#snapshot.revision + 1,
      telemetryPoints: this.#snapshot.telemetryPoints + points.length,
      diagnostics: {
        ...this.#snapshot.diagnostics,
        inboundBytes: this.#snapshot.diagnostics.inboundBytes + points.length * 4,
        validFrames: this.#snapshot.diagnostics.validFrames + 1,
      },
    };
    this.#publish({ event: "snapshotChanged", data: this.#snapshot });
  }

  #complete(message: string): OperationResult {
    return this.#operationResult("succeeded", message);
  }

  #fail(message: string): OperationResult {
    return this.#operationResult("failed", message);
  }

  #operationResult(status: "succeeded" | "failed", message: string): OperationResult {
    const result: OperationResult = {
      operationId: (this.#operationId += 1),
      status,
      message,
    };
    this.#publish({ event: "operationCompleted", data: result });
    return result;
  }

  #publish(event: UnindexedBridgeEvent): void {
    const ordered = { ...event, eventIndex: (this.#eventIndex += 1) } as BridgeEvent;
    for (const listener of this.#listeners) listener(ordered);
  }

  #publishSnapshot(): void {
    this.#publish({ event: "snapshotChanged", data: this.#snapshot });
  }

  #replaceParameter(index: number, parameter: ParameterSnapshot): void {
    const parameters = [...this.#snapshot.parameters];
    parameters[index] = parameter;
    this.#snapshot = {
      ...this.#snapshot,
      revision: this.#snapshot.revision + 1,
      parameters,
      dirtyCount: parameters.filter(({ dirty }) => dirty).length,
      diagnostics: {
        ...this.#snapshot.diagnostics,
        outboundBytes: this.#snapshot.diagnostics.outboundBytes + 32,
        inboundBytes: this.#snapshot.diagnostics.inboundBytes + 32,
      },
    };
    this.#publishSnapshot();
  }

  #writeDenial(): string | null {
    if (this.#snapshot.phase !== "ready") return "设备未连接";
    if (this.#snapshot.accessProfile.role === "observer") return "仅观察者不能修改参数";
    if (!this.#snapshot.accessProfile.leaseActive) return "当前车辆控制权未激活";
    return null;
  }

  #applyDisconnect(reason: string): void {
    this.#snapshot = {
      ...this.#snapshot,
      revision: this.#snapshot.revision + 1,
      phase: "disconnected",
      sessionId: null,
      activeSubscription: null,
      paused: true,
      lastDisconnectReason: reason,
      parameters: this.#snapshot.parameters.map((parameter) => ({
        ...parameter,
        syncKnown: false,
        writeState: "idle",
      })),
    };
    this.#publishSnapshot();
  }
}

function sameValue(left: ParameterValue, right: ParameterValue | null): boolean {
  return right !== null && left.kind === right.kind && Object.is(left.value, right.value);
}

function deterministicValue(slot: number, sequence: number): TelemetryValue {
  switch (slot % 4) {
    case 0:
      return { kind: "f32", value: sequence / 10 + slot };
    case 1:
      return { kind: "i32", value: sequence - slot };
    case 2:
      return { kind: "u32", value: sequence + slot };
    default:
      return { kind: "flags32", value: (sequence + slot) & 0xff };
  }
}
