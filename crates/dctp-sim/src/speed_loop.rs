const CONTROL_STEP_US: u64 = 2_000;
const CONTROL_DT_S: f32 = 0.002;
pub(crate) const MAX_SPEED_MPS: f32 = 4.0;
const PLANT_TAU_S: f32 = 0.25;
const DERIVATIVE_TAU_S: f32 = 0.03;
const INTEGRAL_LIMIT: f32 = 4.0;

#[derive(Clone, Copy, Debug)]
pub(crate) struct SpeedLoopInput {
    pub(crate) target_mps: f32,
    pub(crate) kp: f32,
    pub(crate) ki: f32,
    pub(crate) kd: f32,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct SpeedLoopSnapshot {
    pub(crate) speed_mps: f32,
    pub(crate) error_mps: f32,
    pub(crate) motor_output: f32,
}

#[derive(Debug, Default)]
pub(crate) struct SpeedLoopModel {
    timestamp_us: u64,
    speed_mps: f32,
    previous_speed_mps: f32,
    integral: f32,
    filtered_derivative: f32,
    motor_output: f32,
}

impl SpeedLoopModel {
    pub(crate) fn reset(&mut self) {
        *self = Self::default();
    }

    pub(crate) fn advance_to(&mut self, timestamp_us: u64, input: SpeedLoopInput) {
        let input = input.sanitized();
        while self.timestamp_us.saturating_add(CONTROL_STEP_US) <= timestamp_us {
            self.step(input);
            self.timestamp_us = self.timestamp_us.saturating_add(CONTROL_STEP_US);
        }
    }

    pub(crate) fn snapshot(&self, input: SpeedLoopInput) -> SpeedLoopSnapshot {
        let input = input.sanitized();
        SpeedLoopSnapshot {
            speed_mps: finite_or_zero(self.speed_mps),
            error_mps: finite_or_zero(input.target_mps - self.speed_mps),
            motor_output: finite_or_zero(self.motor_output).clamp(-1.0, 1.0),
        }
    }

    fn step(&mut self, input: SpeedLoopInput) {
        let error = input.target_mps - self.speed_mps;
        if input.target_mps.abs() <= f32::EPSILON {
            self.integral = 0.0;
        }

        let measured_derivative = (self.speed_mps - self.previous_speed_mps) / CONTROL_DT_S;
        let derivative_alpha = 1.0 - (-CONTROL_DT_S / DERIVATIVE_TAU_S).exp();
        self.filtered_derivative +=
            derivative_alpha * (measured_derivative - self.filtered_derivative);

        let integral_candidate =
            (self.integral + error * CONTROL_DT_S).clamp(-INTEGRAL_LIMIT, INTEGRAL_LIMIT);
        let candidate_output =
            input.kp * error + input.ki * integral_candidate - input.kd * self.filtered_derivative;
        let drives_farther_into_saturation =
            (candidate_output > 1.0 && error > 0.0) || (candidate_output < -1.0 && error < 0.0);
        if !drives_farther_into_saturation && input.target_mps.abs() > f32::EPSILON {
            self.integral = integral_candidate;
        }

        self.motor_output = finite_or_zero(
            input.kp * error + input.ki * self.integral - input.kd * self.filtered_derivative,
        )
        .clamp(-1.0, 1.0);

        self.previous_speed_mps = self.speed_mps;
        let plant_alpha = 1.0 - (-CONTROL_DT_S / PLANT_TAU_S).exp();
        let commanded_speed = self.motor_output * MAX_SPEED_MPS;
        self.speed_mps =
            finite_or_zero(self.speed_mps + plant_alpha * (commanded_speed - self.speed_mps))
                .clamp(-MAX_SPEED_MPS, MAX_SPEED_MPS);
    }
}

impl SpeedLoopInput {
    fn sanitized(self) -> Self {
        Self {
            target_mps: finite_or_zero(self.target_mps),
            kp: finite_or_zero(self.kp).max(0.0),
            ki: finite_or_zero(self.ki).max(0.0),
            kd: finite_or_zero(self.kd).max(0.0),
        }
    }
}

fn finite_or_zero(value: f32) -> f32 {
    if value.is_finite() {
        value
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_input(target_mps: f32) -> SpeedLoopInput {
        SpeedLoopInput {
            target_mps,
            kp: 1.2,
            ki: 0.08,
            kd: 0.002,
        }
    }

    #[test]
    fn zero_target_stays_stopped_and_finite() {
        let mut model = SpeedLoopModel::default();
        let input = default_input(0.0);

        model.reset();
        model.advance_to(3_000_000, input);
        let state = model.snapshot(input);

        assert_eq!(state.speed_mps, 0.0);
        assert_eq!(state.error_mps, 0.0);
        assert!(state.motor_output.is_finite());
    }

    #[test]
    fn default_step_rises_and_reaches_a_stable_finite_response() {
        let mut model = SpeedLoopModel::default();
        let input = default_input(1.0);

        model.advance_to(500_000, input);
        let early = model.snapshot(input).speed_mps;
        model.advance_to(3_000_000, input);
        let late = model.snapshot(input).speed_mps;

        assert!(early > 0.1 && early < late, "early={early}, late={late}");
        assert!(late > 0.75 && late <= MAX_SPEED_MPS, "late={late}");
    }

    #[test]
    fn zero_target_clears_integrator_between_repeated_steps() {
        let mut model = SpeedLoopModel::default();
        let run = SpeedLoopInput {
            target_mps: 1.0,
            kp: 0.6,
            ki: 0.5,
            kd: 0.01,
        };

        model.advance_to(3_000_000, run);
        let stop = SpeedLoopInput {
            target_mps: 0.0,
            ..run
        };
        model.advance_to(3_800_000, stop);
        assert!(model.snapshot(stop).speed_mps.abs() < 0.08);

        model.advance_to(6_800_000, run);
        assert!(model.snapshot(run).speed_mps > 0.7);
    }

    #[test]
    fn output_is_bounded_and_higher_gains_change_the_response() {
        let mut low = SpeedLoopModel::default();
        let low_input = SpeedLoopInput {
            target_mps: 1.0,
            kp: 0.4,
            ki: 0.02,
            kd: 0.0,
        };
        low.advance_to(3_000_000, low_input);

        let mut high = SpeedLoopModel::default();
        let high_input = SpeedLoopInput {
            target_mps: 1.0,
            kp: 1.2,
            ki: 0.5,
            kd: 0.002,
        };
        high.advance_to(3_000_000, high_input);

        let low_state = low.snapshot(low_input);
        let high_state = high.snapshot(high_input);
        assert!(high_state.speed_mps > low_state.speed_mps + 0.05);
        assert!(high_state.motor_output.abs() <= 1.0);
    }
}
