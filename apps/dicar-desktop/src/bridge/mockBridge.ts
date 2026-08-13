import type {
  AppSnapshot,
  AccessProfileId,
  BridgeEvent,
  Endpoint,
  OperationResult,
  ParameterSnapshot,
  ParameterValue,
  SerialPortDescriptor,
  TelemetryDescriptor,
  TelemetryPoint,
  TelemetrySubscriptionRequest,
  TelemetryValue,
  WindowCloseDecision,
} from "../domain/types";
import type { DesktopBridge } from "./desktopBridge";
import { MAX_SPEED_MPS, SpeedLoopModel, type SpeedLoopInput, type SpeedLoopSnapshot } from "../tuning/speedLoopModel";

type BridgeListener = (event: BridgeEvent) => void;
type UnindexedBridgeEvent<T = BridgeEvent> = T extends { eventIndex: number }
  ? Omit<T, "eventIndex">
  : never;

const simulatorTelemetryNames = [
  "drive.speed_mps", "encoder.left_delta", "encoder.left_total", "drive.fault_flags",
  "encoder.right_total", "drive.left_wheel_speed_mps", "drive.right_wheel_speed_mps", "drive.target_speed_mps",
  "drive.speed_error_mps", "motor.left_pwm", "motor.right_pwm", "encoder.right_delta",
  "control.loop_jitter_us", "power.battery_voltage", "steering.error_deg", "system.uptime_ms",
];
const simulatorTelemetryTypes: TelemetryDescriptor["telemetryType"][] = [
  "f32", "i32", "u32", "flags32", "u32", "f32", "f32", "f32",
  "f32", "u32", "u32", "i32", "u32", "f32", "f32", "u32",
];

const telemetryDescriptors: TelemetryDescriptor[] = Array.from({ length: 16 }, (_, index) => ({
  channelId: 200 + index,
  telemetryType: simulatorTelemetryTypes[index],
  machineName: simulatorTelemetryNames[index],
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
    parameters: parameterFixtures(),
    telemetryDescriptors,
    dirtyCount: 0,
    storageGeneration: 0,
    accessProfile: { role: "owner", leaseActive: true, localDemoOnly: true },
    desiredSubscription: null,
    activeSubscription: null,
    linkBudget: null,
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

function parameterFixtures(): ParameterSnapshot[] {
  return [
    numericParameter(1, "pid.kp", "速度环 Kp", "速度环 PID", "", "f32", 1.2, 0, 20, 0.01, "比例增益，影响速度误差的即时响应"),
    numericParameter(2, "pid.speed.ki", "速度环 Ki", "速度环 PID", "", "f32", 0.08, 0, 5, 0.001, "积分增益，用于消除稳态误差"),
    numericParameter(3, "pid.speed.kd", "速度环 Kd", "速度环 PID", "", "f32", 0.002, 0, 1, 0.0001, "微分增益，用于抑制快速变化"),
    numericParameter(4, "control.target_speed_mps", "目标速度", "速度环 PID", "m/s", "f32", 0, 0, 8, 0.05, "测试阶段目标车速", true, false),
    numericParameter(100, "encoder.left.ppr", "左编码器 PPR", "编码器与车轮", "pulse/rev", "u32", 512, 1, 65535, 1, "左编码器每机械转输出的脉冲数"),
    numericParameter(101, "encoder.right.ppr", "右编码器 PPR", "编码器与车轮", "pulse/rev", "u32", 512, 1, 65535, 1, "右编码器每机械转输出的脉冲数"),
    enumParameter(102, "encoder.quadrature_multiplier", "正交倍频", "编码器与车轮", 4, [{ value: 1, label: "×1" }, { value: 2, label: "×2" }, { value: 4, label: "×4" }]),
    boolParameter(103, "encoder.left.inverted", "左侧方向反向", "编码器与车轮", false),
    boolParameter(104, "encoder.right.inverted", "右侧方向反向", "编码器与车轮", true),
    numericParameter(105, "drive.wheel_diameter_mm", "车轮直径", "编码器与车轮", "mm", "f32", 64, 10, 200, 0.1),
    numericParameter(106, "drive.gear_ratio", "传动比", "编码器与车轮", "ratio", "f32", 1, 0.01, 20, 0.001),
    numericParameter(107, "encoder.sample_period_us", "测速采样周期", "编码器与车轮", "µs", "u32", 2000, 100, 100000, 100),
    numericParameter(108, "encoder.speed_lpf_hz", "速度低通截止频率", "编码器与车轮", "Hz", "f32", 35, 0.1, 250, 0.1),
    numericParameter(109, "encoder.jump_threshold_counts", "计数跳变阈值", "编码器与车轮", "count", "u32", 240, 1, 10000, 1),
    numericParameter(110, "encoder.max_credible_rpm", "可信最高转速", "编码器与车轮", "rpm", "u32", 6000, 1, 100000, 10),
    boolParameter(111, "encoder.missing_pulse_detection", "缺脉冲检测", "编码器与车轮", true),
    numericParameter(120, "motor.pwm_limit", "电机 PWM 上限", "电机与保护", "%", "u32", 92, 0, 100, 1, "限制驱动输出，修改前确认电源和机械安全", true),
    numericParameter(121, "motor.current_limit_a", "电机电流上限", "电机与保护", "A", "f32", 12, 0, 40, 0.1, "超过此阈值时触发保护", true),
    numericParameter(122, "steering.center_pwm_us", "舵机中位 PWM", "转向控制", "µs", "u32", 1500, 800, 2200, 1),
  ];
}

function numericParameter(
  paramId: number,
  machineName: string,
  displayName: string,
  group: string,
  unit: string,
  kind: "f32" | "i32" | "u32",
  value: number,
  min: number,
  max: number,
  step: number,
  description?: string,
  dangerous = false,
  persistent = true,
): ParameterSnapshot {
  const parameterValue = { kind, value } as ParameterValue;
  return { paramId, machineName, displayName, group, unit, ramValue: parameterValue, persistedValue: persistent ? parameterValue : null, revision: 1, dirty: false, syncKnown: false, writeState: "idle", writable: true, dangerous, lastError: null, description, numeric: { min, max, step } };
}

function boolParameter(paramId: number, machineName: string, displayName: string, group: string, value: boolean): ParameterSnapshot {
  const parameterValue = { kind: "bool", value } as const;
  return { paramId, machineName, displayName, group, unit: "", ramValue: parameterValue, persistedValue: parameterValue, revision: 1, dirty: false, syncKnown: false, writeState: "idle", writable: true, dangerous: false, lastError: null };
}

function enumParameter(paramId: number, machineName: string, displayName: string, group: string, value: number, enumOptions: Array<{ value: number; label: string }>): ParameterSnapshot {
  const parameterValue = { kind: "enum", value } as const;
  return { paramId, machineName, displayName, group, unit: "", ramValue: parameterValue, persistedValue: parameterValue, revision: 1, dirty: false, syncKnown: false, writeState: "idle", writable: true, dangerous: false, lastError: null, enumOptions };
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
  #speedLoop = new SpeedLoopModel();
  #scheduler: ReturnType<typeof setInterval> | null = null;
  #schedulerLastMs = 0;
  #schedulerCarryUs = 0;

  async listSerialPorts(): Promise<SerialPortDescriptor[]> {
    throw new Error("当前 Web 预览不能访问真实串口，请使用桌面 App");
  }

  async subscribe(listener: BridgeListener): Promise<() => void> {
    this.#listeners.add(listener);
    this.#reconcileScheduler();
    return () => {
      this.#listeners.delete(listener);
      this.#reconcileScheduler();
    };
  }

  async connect(endpoint: Endpoint): Promise<OperationResult> {
    if (endpoint.kind === "serial") {
      return this.#fail("当前 Web 预览不能访问真实串口，请使用桌面 App");
    }
    this.#sampleSequence = 0;
    this.#timestampUs = 0;
    this.#speedLoop.reset();
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
      linkBudget: {
        hardwareProfile: null,
        baudRate: null,
        maxChannels: 8,
        maxSampleRateHz: 500,
        reason: "内置模拟器支持完整 8 通道 × 500 Hz 遥测",
      },
    };
    this.#snapshot.desiredSubscription = this.#snapshot.activeSubscription;
    this.#publish({ event: "snapshotChanged", data: this.#snapshot });
    this.advanceTelemetry(200);
    this.#restartScheduler();
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
    if (value.kind !== "bool") {
      if (!Number.isFinite(value.value)) return this.#fail("参数值必须是有限数值");
      if ((value.kind === "i32" || value.kind === "u32" || value.kind === "enum") && !Number.isInteger(value.value)) {
        return this.#fail("整数参数不能包含小数");
      }
      if (current.numeric !== undefined && (value.value < current.numeric.min || value.value > current.numeric.max)) {
        return this.#fail(`参数值必须在 ${current.numeric.min}–${current.numeric.max} 范围内`);
      }
      if (current.enumOptions !== undefined && !current.enumOptions.some((option) => option.value === value.value)) {
        return this.#fail("枚举参数值无效");
      }
    }
    this.#history.push({ paramId, value: current.ramValue });
    const next: ParameterSnapshot = {
      ...current,
      ramValue: value,
      revision: (current.revision + 1) >>> 0,
      dirty: current.persistedValue !== null && !sameValue(value, current.persistedValue),
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
    const maxChannels = this.#snapshot.linkBudget?.maxChannels ?? 8;
    const maxSampleRateHz = this.#snapshot.linkBudget?.maxSampleRateHz ?? 500;
    if (
      request.channelIds.length === 0 ||
      request.channelIds.length > maxChannels ||
      unique.size !== request.channelIds.length ||
      request.channelIds.some((channelId) => !known.has(channelId)) ||
      request.sampleRateHz < 1 ||
      request.sampleRateHz > maxSampleRateHz
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
    this.advanceTelemetry(50);
    this.#restartScheduler();
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
    this.#restartScheduler();
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
    const periodUs = Math.floor(1_000_000 / subscription.sampleRateHz);
    const input = this.#speedLoopInput();
    for (let sample = 0; sample < sampleCount; sample += 1) {
      this.#timestampUs += periodUs;
      this.#speedLoop.advanceTo(this.#timestampUs, input);
      const state = this.#speedLoop.snapshot(input);
      for (const [slot, channelId] of subscription.channelIds.entries()) {
        const descriptor = telemetryDescriptors.find((candidate) => candidate.channelId === channelId);
        points.push({
          channelId,
          timestampUs: this.#timestampUs,
          sampleSequence: this.#sampleSequence & 0xffff,
          value: descriptor === undefined
            ? deterministicValue("u32", slot, this.#sampleSequence)
            : telemetryValue(descriptor, input, state, slot, this.#sampleSequence),
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
    this.#stopScheduler();
    this.#snapshot = {
      ...this.#snapshot,
      revision: this.#snapshot.revision + 1,
      phase: "disconnected",
      sessionId: null,
      activeSubscription: null,
      linkBudget: null,
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

  async clearTelemetrySubscription(): Promise<OperationResult> {
    if (this.#snapshot.phase !== "ready") return this.#fail("设备未连接");
    this.#snapshot = {
      ...this.#snapshot,
      revision: this.#snapshot.revision + 1,
      desiredSubscription: null,
      activeSubscription: null,
      paused: true,
    };
    this.#publishSnapshot();
    this.#stopScheduler();
    return this.#complete("遥测订阅已清除");
  }

  #speedLoopInput(): SpeedLoopInput {
    const value = (machineName: string, fallback: number) => {
      const parameter = this.#snapshot.parameters.find((record) => record.machineName === machineName);
      return parameter !== undefined && parameter.ramValue.kind !== "bool" && Number.isFinite(parameter.ramValue.value)
        ? parameter.ramValue.value
        : fallback;
    };
    return {
      targetMps: value("control.target_speed_mps", 0),
      kp: value("pid.kp", 1.2),
      ki: value("pid.speed.ki", 0.08),
      kd: value("pid.speed.kd", 0.002),
    };
  }

  #restartScheduler(): void {
    this.#stopScheduler();
    this.#reconcileScheduler();
  }

  #reconcileScheduler(): void {
    const shouldRun = this.#snapshot.phase === "ready"
      && this.#snapshot.activeSubscription !== null
      && !this.#snapshot.paused
      && this.#listeners.size > 0;
    if (!shouldRun) {
      this.#stopScheduler();
      return;
    }
    if (this.#scheduler !== null) return;
    this.#schedulerLastMs = Date.now();
    this.#schedulerCarryUs = 0;
    this.#scheduler = setInterval(() => this.#runScheduler(), 20);
  }

  #runScheduler(): void {
    const subscription = this.#snapshot.activeSubscription;
    if (subscription === null || this.#snapshot.phase !== "ready" || this.#snapshot.paused || this.#listeners.size === 0) {
      this.#reconcileScheduler();
      return;
    }
    const nowMs = Date.now();
    this.#schedulerCarryUs += Math.max(0, nowMs - this.#schedulerLastMs) * 1_000;
    this.#schedulerLastMs = nowMs;
    const periodUs = Math.floor(1_000_000 / subscription.sampleRateHz);
    const due = Math.floor(this.#schedulerCarryUs / periodUs);
    if (due <= 0) return;
    this.#schedulerCarryUs -= due * periodUs;
    this.advanceTelemetry(Math.min(due, 500));
  }

  #stopScheduler(): void {
    if (this.#scheduler !== null) clearInterval(this.#scheduler);
    this.#scheduler = null;
    this.#schedulerCarryUs = 0;
  }
}

function sameValue(left: ParameterValue, right: ParameterValue | null): boolean {
  return right !== null && left.kind === right.kind && Object.is(left.value, right.value);
}

function deterministicValue(kind: TelemetryDescriptor["telemetryType"], slot: number, sequence: number): TelemetryValue {
  switch (kind) {
    case "f32": return { kind, value: sequence / 10 + slot };
    case "i32": return { kind, value: sequence - slot };
    case "u32": return { kind, value: sequence + slot };
    case "flags32": return { kind, value: (sequence + slot) & 0xff };
  }
}

function telemetryValue(
  descriptor: TelemetryDescriptor,
  input: SpeedLoopInput,
  state: SpeedLoopSnapshot,
  slot: number,
  sequence: number,
): TelemetryValue {
  switch (descriptor.machineName) {
    case "drive.speed_mps": return { kind: "f32", value: state.speedMps };
    case "drive.left_wheel_speed_mps": return { kind: "f32", value: clamp(state.speedMps * 0.99, -MAX_SPEED_MPS, MAX_SPEED_MPS) };
    case "drive.right_wheel_speed_mps": return { kind: "f32", value: clamp(state.speedMps * 1.01, -MAX_SPEED_MPS, MAX_SPEED_MPS) };
    case "drive.target_speed_mps": return { kind: "f32", value: input.targetMps };
    case "drive.speed_error_mps": return { kind: "f32", value: state.errorMps };
    case "motor.left_pwm":
    case "motor.right_pwm": return { kind: "u32", value: Math.round(Math.abs(clamp(state.motorOutput, -1, 1)) * 1_000) };
    default: return deterministicValue(descriptor.telemetryType, slot, sequence);
  }
}

function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}
