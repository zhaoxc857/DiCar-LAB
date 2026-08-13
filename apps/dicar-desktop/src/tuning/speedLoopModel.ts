const CONTROL_STEP_US = 2_000;
const CONTROL_DT_S = 0.002;
export const MAX_SPEED_MPS = 4;
const PLANT_TAU_S = 0.25;
const DERIVATIVE_TAU_S = 0.03;
const INTEGRAL_LIMIT = 4;

export interface SpeedLoopInput {
  targetMps: number;
  kp: number;
  ki: number;
  kd: number;
}

export interface SpeedLoopSnapshot {
  speedMps: number;
  errorMps: number;
  motorOutput: number;
}

export class SpeedLoopModel {
  private timestampUs = 0;
  private speedMps = 0;
  private previousSpeedMps = 0;
  private integral = 0;
  private filteredDerivative = 0;
  private motorOutput = 0;

  reset(timestampUs = 0): void {
    this.timestampUs = finiteOrZero(timestampUs);
    this.speedMps = 0;
    this.previousSpeedMps = 0;
    this.integral = 0;
    this.filteredDerivative = 0;
    this.motorOutput = 0;
  }

  advanceTo(timestampUs: number, input: SpeedLoopInput): void {
    const safeTimestampUs = Math.max(0, finiteOrZero(timestampUs));
    const safeInput = sanitize(input);
    while (this.timestampUs + CONTROL_STEP_US <= safeTimestampUs) {
      this.step(safeInput);
      this.timestampUs += CONTROL_STEP_US;
    }
  }

  snapshot(input: SpeedLoopInput): SpeedLoopSnapshot {
    const safeInput = sanitize(input);
    return {
      speedMps: finiteOrZero(this.speedMps),
      errorMps: finiteOrZero(safeInput.targetMps - this.speedMps),
      motorOutput: clamp(finiteOrZero(this.motorOutput), -1, 1),
    };
  }

  private step(input: SpeedLoopInput): void {
    const error = input.targetMps - this.speedMps;
    if (Math.abs(input.targetMps) <= Number.EPSILON) {
      this.integral = 0;
    }

    const measuredDerivative = (this.speedMps - this.previousSpeedMps) / CONTROL_DT_S;
    const derivativeAlpha = 1 - Math.exp(-CONTROL_DT_S / DERIVATIVE_TAU_S);
    this.filteredDerivative += derivativeAlpha * (measuredDerivative - this.filteredDerivative);

    const integralCandidate = clamp(this.integral + error * CONTROL_DT_S, -INTEGRAL_LIMIT, INTEGRAL_LIMIT);
    const candidateOutput = input.kp * error + input.ki * integralCandidate - input.kd * this.filteredDerivative;
    const drivesFartherIntoSaturation =
      (candidateOutput > 1 && error > 0) || (candidateOutput < -1 && error < 0);
    if (!drivesFartherIntoSaturation && Math.abs(input.targetMps) > Number.EPSILON) {
      this.integral = integralCandidate;
    }

    this.motorOutput = clamp(
      finiteOrZero(input.kp * error + input.ki * this.integral - input.kd * this.filteredDerivative),
      -1,
      1,
    );

    this.previousSpeedMps = this.speedMps;
    const plantAlpha = 1 - Math.exp(-CONTROL_DT_S / PLANT_TAU_S);
    const commandedSpeed = this.motorOutput * MAX_SPEED_MPS;
    this.speedMps = clamp(
      finiteOrZero(this.speedMps + plantAlpha * (commandedSpeed - this.speedMps)),
      -MAX_SPEED_MPS,
      MAX_SPEED_MPS,
    );
  }
}

function sanitize(input: SpeedLoopInput): SpeedLoopInput {
  return {
    targetMps: finiteOrZero(input.targetMps),
    kp: Math.max(0, finiteOrZero(input.kp)),
    ki: Math.max(0, finiteOrZero(input.ki)),
    kd: Math.max(0, finiteOrZero(input.kd)),
  };
}

function finiteOrZero(value: number): number {
  return Number.isFinite(value) ? value : 0;
}

function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}
