use dctp_protocol::{Frame, FrameFlags, MessageType};
use dctp_sim::{Priority, PriorityTxQueue, PushOutcome};

fn frame(sequence: u16) -> Frame {
    Frame::new(
        MessageType::Heartbeat,
        FrameFlags::NONE,
        sequence,
        1,
        vec![],
    )
    .unwrap()
}

#[test]
fn control_frames_precede_telemetry_and_logs() {
    let mut queue = PriorityTxQueue::with_capacities([4, 4, 2, 2]);
    queue.push(Priority::Log, frame(1));
    queue.push(Priority::Telemetry, frame(2));
    queue.push(Priority::Reliable, frame(3));
    queue.push(Priority::Safety, frame(4));

    assert_eq!(queue.pop().unwrap().header.sequence, 4);
    assert_eq!(queue.pop().unwrap().header.sequence, 3);
    assert_eq!(queue.pop().unwrap().header.sequence, 2);
    assert_eq!(queue.pop().unwrap().header.sequence, 1);
}

#[test]
fn full_log_queue_drops_new_frame_and_counts_it() {
    let mut queue = PriorityTxQueue::with_capacities([1, 1, 1, 1]);
    assert_eq!(queue.push(Priority::Log, frame(1)), PushOutcome::Enqueued);
    assert_eq!(queue.push(Priority::Log, frame(2)), PushOutcome::DroppedLog);
    assert_eq!(queue.dropped_logs(), 1);
    assert_eq!(queue.pop().unwrap().header.sequence, 1);
}

#[test]
fn full_telemetry_queue_drops_oldest_complete_frame_and_counts_it() {
    let mut queue = PriorityTxQueue::with_capacities([1, 1, 2, 1]);
    queue.push(Priority::Telemetry, frame(1));
    queue.push(Priority::Telemetry, frame(2));

    assert_eq!(
        queue.push(Priority::Telemetry, frame(3)),
        PushOutcome::DroppedTelemetry
    );
    assert_eq!(queue.dropped_telemetry(), 1);
    assert_eq!(queue.pop().unwrap().header.sequence, 2);
    assert_eq!(queue.pop().unwrap().header.sequence, 3);
}

#[test]
fn full_safety_and_reliable_queues_apply_backpressure_without_eviction() {
    for priority in [Priority::Safety, Priority::Reliable] {
        let mut queue = PriorityTxQueue::with_capacities([1, 1, 1, 1]);
        assert_eq!(queue.push(priority, frame(1)), PushOutcome::Enqueued);
        assert_eq!(queue.push(priority, frame(2)), PushOutcome::Backpressure);
        assert_eq!(queue.pop().unwrap().header.sequence, 1);
        assert!(queue.pop().is_none());
    }
}

#[test]
fn zero_capacity_never_panics_and_obeys_each_priority_policy() {
    let mut queue = PriorityTxQueue::with_capacities([0, 0, 0, 0]);

    assert_eq!(
        queue.push(Priority::Safety, frame(1)),
        PushOutcome::Backpressure
    );
    assert_eq!(
        queue.push(Priority::Reliable, frame(2)),
        PushOutcome::Backpressure
    );
    assert_eq!(
        queue.push(Priority::Telemetry, frame(3)),
        PushOutcome::DroppedTelemetry
    );
    assert_eq!(queue.push(Priority::Log, frame(4)), PushOutcome::DroppedLog);
    assert_eq!(queue.dropped_telemetry(), 1);
    assert_eq!(queue.dropped_logs(), 1);
    assert!(queue.pop().is_none());
}
