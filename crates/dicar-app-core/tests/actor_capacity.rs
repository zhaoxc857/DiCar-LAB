use std::time::{Duration, Instant};

use dctp_sim::SimulatorServer;
use dicar_app_core::{
    ActorSendError, AppActorHandle, CoreCommand, CoreConfig, CoreEventPayload, SnapshotPhase,
};

fn wait_for(timeout: Duration, predicate: impl Fn() -> bool) {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if predicate() {
            return;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("condition did not become true within {timeout:?}");
}

#[test]
fn command_queue_reports_overload_within_two_milliseconds_and_shutdown_joins() {
    let actor = AppActorHandle::spawn(
        CoreConfig::simulator("127.0.0.1:9".parse().unwrap())
            .with_command_capacity(2)
            .with_startup_delay(Duration::from_millis(100)),
    )
    .unwrap();
    actor.send(CoreCommand::GetSnapshot).unwrap();
    actor.send(CoreCommand::GetSnapshot).unwrap();
    let started = Instant::now();
    assert_eq!(
        actor.send(CoreCommand::GetSnapshot),
        Err(ActorSendError::Overloaded { capacity: 2 })
    );
    assert!(started.elapsed() < Duration::from_millis(2));

    let shutdown_started = Instant::now();
    actor.shutdown().unwrap();
    assert!(shutdown_started.elapsed() < Duration::from_millis(500));
}

#[test]
fn stalled_ui_keeps_one_snapshot_and_four_whole_telemetry_batches() {
    let server = SimulatorServer::spawn("127.0.0.1:0".parse().unwrap()).unwrap();
    let actor = AppActorHandle::spawn(CoreConfig::simulator(server.local_addr())).unwrap();
    let events = actor.subscribe().unwrap();
    actor.send(CoreCommand::Connect).unwrap();
    wait_for(Duration::from_secs(2), || {
        actor.snapshot().phase == SnapshotPhase::Ready
    });
    let channel_ids = actor
        .snapshot()
        .telemetry_descriptors
        .iter()
        .take(8)
        .map(|descriptor| descriptor.channel_id)
        .collect();
    actor
        .send(CoreCommand::SetTelemetrySubscription {
            channel_ids,
            sample_rate_hz: 500,
        })
        .unwrap();
    wait_for(Duration::from_secs(1), || {
        actor.snapshot().active_subscription.is_some()
    });
    wait_for(Duration::from_secs(2), || {
        actor.snapshot().diagnostics.ui_dropped_batches > 0
    });
    actor.send(CoreCommand::SetPaused { paused: true }).unwrap();
    wait_for(Duration::from_secs(1), || actor.snapshot().paused);

    let pending = events.drain();
    let telemetry = pending
        .iter()
        .filter(|event| matches!(event.payload, CoreEventPayload::TelemetryBatch(_)))
        .count();
    let snapshots = pending
        .iter()
        .filter(|event| matches!(event.payload, CoreEventPayload::SnapshotChanged(_)))
        .count();
    assert!(telemetry <= 4);
    assert!(snapshots <= 1);
    assert!(actor.snapshot().diagnostics.ui_dropped_batches > 0);
    assert!(actor.snapshot().telemetry_points <= 240_000);

    actor.shutdown().unwrap();
    server.shutdown().unwrap();
}

#[test]
fn flowing_telemetry_is_published_at_no_more_than_the_default_visual_rate() {
    let server = SimulatorServer::spawn("127.0.0.1:0".parse().unwrap()).unwrap();
    let actor = AppActorHandle::spawn(CoreConfig::simulator(server.local_addr())).unwrap();
    let events = actor.subscribe().unwrap();
    actor.send(CoreCommand::Connect).unwrap();
    wait_for(Duration::from_secs(2), || {
        actor.snapshot().phase == SnapshotPhase::Ready
    });
    let channel_ids = actor
        .snapshot()
        .telemetry_descriptors
        .iter()
        .take(8)
        .map(|descriptor| descriptor.channel_id)
        .collect();
    actor
        .send(CoreCommand::SetTelemetrySubscription {
            channel_ids,
            sample_rate_hz: 500,
        })
        .unwrap();
    wait_for(Duration::from_secs(1), || {
        actor.snapshot().active_subscription.is_some()
    });

    let alignment_deadline = Instant::now() + Duration::from_secs(1);
    while Instant::now() < alignment_deadline {
        if events
            .recv_timeout(Duration::from_millis(20))
            .is_ok_and(|event| matches!(event.payload, CoreEventPayload::TelemetryBatch(_)))
        {
            break;
        }
    }
    events.drain();

    let window = Duration::from_millis(400);
    let started = Instant::now();
    let mut visual_batches = 0;
    while started.elapsed() < window {
        if events
            .recv_timeout(Duration::from_millis(10))
            .is_ok_and(|event| matches!(event.payload, CoreEventPayload::TelemetryBatch(_)))
        {
            visual_batches += 1;
        }
    }
    assert!(
        visual_batches <= 13,
        "default UI batching exceeded the 30 Hz window: {visual_batches}"
    );

    actor.shutdown().unwrap();
    server.shutdown().unwrap();
}

#[test]
fn reliable_event_overrun_is_explicit_and_terminal_instead_of_blocking() {
    let actor =
        AppActorHandle::spawn(CoreConfig::simulator("127.0.0.1:9".parse().unwrap())).unwrap();
    let events = actor.subscribe().unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut accepted = Vec::new();
    while Instant::now() < deadline {
        match actor.send(CoreCommand::GetSnapshot) {
            Ok(operation_id) => accepted.push(operation_id),
            Err(ActorSendError::Overloaded { .. }) => std::thread::yield_now(),
            Err(ActorSendError::Closed) => break,
        }
    }
    assert!(accepted.len() >= 65);
    wait_for(Duration::from_secs(1), || {
        actor.send(CoreCommand::GetSnapshot) == Err(ActorSendError::Closed)
    });

    let drained = events.drain();
    let fatal = drained
        .iter()
        .find_map(|event| match &event.payload {
            CoreEventPayload::FatalError(error) if error.code == "frontendOverrun" => Some(error),
            _ => None,
        })
        .expect("frontend overrun must remain observable");
    assert!(fatal
        .operation_id
        .is_some_and(|operation_id| accepted.contains(&operation_id)));
    assert!(drained
        .windows(2)
        .all(|pair| pair[0].actor_order < pair[1].actor_order));

    actor.shutdown().unwrap();
}
