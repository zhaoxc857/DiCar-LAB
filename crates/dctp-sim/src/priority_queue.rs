use std::collections::VecDeque;

use dctp_protocol::Frame;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Priority {
    Safety,
    Reliable,
    Telemetry,
    Log,
}

impl Priority {
    const fn index(self) -> usize {
        match self {
            Self::Safety => 0,
            Self::Reliable => 1,
            Self::Telemetry => 2,
            Self::Log => 3,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PushOutcome {
    Enqueued,
    DroppedTelemetry,
    DroppedLog,
    Backpressure,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QueuedFrame {
    pub priority: Priority,
    pub frame: Frame,
}

#[derive(Debug)]
pub struct PriorityTxQueue {
    queues: [VecDeque<Frame>; 4],
    capacities: [usize; 4],
    dropped_telemetry: u64,
    dropped_logs: u64,
}

impl Default for PriorityTxQueue {
    fn default() -> Self {
        Self::with_capacities([8, 32, 16, 16])
    }
}

impl PriorityTxQueue {
    pub fn with_capacities(capacities: [usize; 4]) -> Self {
        Self {
            queues: std::array::from_fn(|index| VecDeque::with_capacity(capacities[index])),
            capacities,
            dropped_telemetry: 0,
            dropped_logs: 0,
        }
    }

    pub fn push(&mut self, priority: Priority, frame: Frame) -> PushOutcome {
        let index = priority.index();
        if self.queues[index].len() < self.capacities[index] {
            self.queues[index].push_back(frame);
            return PushOutcome::Enqueued;
        }

        match priority {
            Priority::Safety | Priority::Reliable => PushOutcome::Backpressure,
            Priority::Telemetry => {
                self.dropped_telemetry = self.dropped_telemetry.saturating_add(1);
                if self.capacities[index] != 0 {
                    self.queues[index].pop_front();
                    self.queues[index].push_back(frame);
                }
                PushOutcome::DroppedTelemetry
            }
            Priority::Log => {
                self.dropped_logs = self.dropped_logs.saturating_add(1);
                PushOutcome::DroppedLog
            }
        }
    }

    pub fn pop(&mut self) -> Option<Frame> {
        self.queues.iter_mut().find_map(VecDeque::pop_front)
    }

    pub const fn dropped_telemetry(&self) -> u64 {
        self.dropped_telemetry
    }

    pub const fn dropped_logs(&self) -> u64 {
        self.dropped_logs
    }
}
