use std::net::{Shutdown, TcpListener};
use std::time::{Duration, Instant};

use dctp_sim::SimulatorServer;
use dicar_app_core::{
    AppActorHandle, CoreCommand, CoreConfig, CoreEventPayload, Endpoint, OperationStatus,
    ParamValueDto, SnapshotPhase,
};

fn wait_until(timeout: Duration, predicate: impl Fn() -> bool) {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if predicate() {
            return;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("actor condition did not become true within {timeout:?}");
}

#[test]
fn failed_protocol_handshake_does_not_leave_a_connected_transport_identity() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let peer = std::thread::spawn(move || {
        let (socket, _) = listener.accept().unwrap();
        socket.shutdown(Shutdown::Both).unwrap();
    });
    let actor = AppActorHandle::spawn(CoreConfig::simulator(address)).unwrap();
    let events = actor.subscribe().unwrap();

    let operation_id = actor
        .send(CoreCommand::ConnectTo {
            endpoint: Endpoint::Simulator { address },
        })
        .unwrap();
    let result = loop {
        let event = events.recv_timeout(Duration::from_secs(2)).unwrap();
        if let CoreEventPayload::OperationCompleted(result) = event.payload {
            if result.operation_id == operation_id {
                break result;
            }
        }
    };

    assert_eq!(result.status, OperationStatus::Failed);
    assert_eq!(actor.snapshot().phase, SnapshotPhase::Disconnected);
    assert_eq!(actor.snapshot().transport_identity, None);

    actor.shutdown().unwrap();
    peer.join().unwrap();
}

#[test]
fn actor_connects_writes_subscribes_pauses_and_streams_ordered_events() {
    let server = SimulatorServer::spawn("127.0.0.1:0".parse().unwrap()).unwrap();
    let actor = AppActorHandle::spawn(CoreConfig::simulator(server.local_addr())).unwrap();
    let events = actor.subscribe().unwrap();

    let connect_id = actor.send(CoreCommand::Connect).unwrap();
    wait_until(Duration::from_secs(2), || {
        actor.snapshot().phase == SnapshotPhase::Ready
    });
    let ready = actor.snapshot();
    assert_eq!(ready.link_budget.as_ref().unwrap().max_channels, 8);
    assert_eq!(ready.link_budget.as_ref().unwrap().max_sample_rate_hz, 500);
    assert!(ready
        .parameters
        .iter()
        .any(|parameter| parameter.machine_name == "encoder.left.ppr"));
    let channel_ids = ready
        .telemetry_descriptors
        .iter()
        .take(8)
        .map(|descriptor| descriptor.channel_id)
        .collect::<Vec<_>>();
    assert_eq!(channel_ids.len(), 8);

    let write_id = actor
        .send(CoreCommand::WriteParameter {
            param_id: 1,
            value: ParamValueDto::F32(2.5),
        })
        .unwrap();
    let subscribe_id = actor
        .send(CoreCommand::SetTelemetrySubscription {
            channel_ids,
            sample_rate_hz: 500,
        })
        .unwrap();

    let deadline = Instant::now() + Duration::from_secs(2);
    let mut actor_orders = Vec::new();
    let mut completed = Vec::new();
    let mut saw_telemetry = false;
    while Instant::now() < deadline && (!saw_telemetry || completed.len() < 3) {
        if let Ok(event) = events.recv_timeout(Duration::from_millis(50)) {
            actor_orders.push(event.actor_order);
            match event.payload {
                CoreEventPayload::OperationCompleted(result) => completed.push(result),
                CoreEventPayload::TelemetryBatch(batch) => {
                    saw_telemetry |= !batch.points.is_empty();
                }
                _ => {}
            }
        }
    }
    assert!(actor_orders.windows(2).all(|pair| pair[0] < pair[1]));
    for operation_id in [connect_id, write_id, subscribe_id] {
        assert!(completed.iter().any(|result| {
            result.operation_id == operation_id && result.status == OperationStatus::Succeeded
        }));
    }
    assert!(saw_telemetry);
    assert_eq!(
        actor
            .snapshot()
            .parameters
            .iter()
            .find(|parameter| parameter.param_id == 1)
            .unwrap()
            .ram_value,
        ParamValueDto::F32(2.5)
    );

    let live_write_id = actor
        .send(CoreCommand::WriteParameter {
            param_id: 1,
            value: ParamValueDto::F32(2.75),
        })
        .unwrap();
    wait_until(Duration::from_secs(1), || {
        actor
            .snapshot()
            .parameters
            .iter()
            .find(|parameter| parameter.param_id == 1)
            .is_some_and(|parameter| parameter.ram_value == ParamValueDto::F32(2.75))
    });
    let live_result = loop {
        let event = events.recv_timeout(Duration::from_secs(1)).unwrap();
        if let CoreEventPayload::OperationCompleted(result) = event.payload {
            if result.operation_id == live_write_id {
                break result;
            }
        }
    };
    assert_eq!(live_result.status, OperationStatus::Succeeded);
    assert!(actor.snapshot().diagnostics.inbound_bytes > 0);
    assert!(actor.snapshot().diagnostics.outbound_bytes > 0);

    let pause_id = actor.send(CoreCommand::SetPaused { paused: true }).unwrap();
    wait_until(Duration::from_secs(1), || actor.snapshot().paused);
    let frozen_points = actor.snapshot().telemetry_points;
    std::thread::sleep(Duration::from_millis(50));
    assert_eq!(actor.snapshot().telemetry_points, frozen_points);
    assert!(events.drain().into_iter().any(|event| matches!(
        event.payload,
        CoreEventPayload::OperationCompleted(ref result)
            if result.operation_id == pause_id && result.status == OperationStatus::Succeeded
    )));

    let paused_version = actor
        .snapshot()
        .desired_subscription
        .as_ref()
        .unwrap()
        .subscription_version;
    actor
        .send(CoreCommand::SetPaused { paused: false })
        .unwrap();
    wait_until(Duration::from_secs(1), || {
        actor
            .snapshot()
            .active_subscription
            .as_ref()
            .is_some_and(|subscription| subscription.subscription_version > paused_version)
    });
    wait_until(Duration::from_secs(1), || {
        actor.snapshot().telemetry_points > frozen_points
    });

    actor.shutdown().unwrap();
    server.shutdown().unwrap();
}

#[test]
fn unexpected_disconnect_marks_parameter_truth_unknown_without_replaying_subscription() {
    let server = SimulatorServer::spawn("127.0.0.1:0".parse().unwrap()).unwrap();
    let actor = AppActorHandle::spawn(CoreConfig::simulator(server.local_addr())).unwrap();
    let events = actor.subscribe().unwrap();
    actor.send(CoreCommand::Connect).unwrap();
    wait_until(Duration::from_secs(2), || {
        actor.snapshot().phase == SnapshotPhase::Ready
    });
    actor
        .send(CoreCommand::WriteParameter {
            param_id: 1,
            value: ParamValueDto::F32(3.5),
        })
        .unwrap();
    wait_until(Duration::from_secs(1), || actor.snapshot().dirty_count == 1);
    let channel_ids = actor
        .snapshot()
        .telemetry_descriptors
        .iter()
        .take(2)
        .map(|descriptor| descriptor.channel_id)
        .collect();
    actor
        .send(CoreCommand::SetTelemetrySubscription {
            channel_ids,
            sample_rate_hz: 100,
        })
        .unwrap();
    wait_until(Duration::from_secs(1), || {
        actor.snapshot().active_subscription.is_some()
    });
    let connected_diagnostics = actor.snapshot().diagnostics;
    assert!(connected_diagnostics.inbound_bytes > 0);
    assert!(connected_diagnostics.outbound_bytes > 0);

    server.shutdown().unwrap();
    wait_until(Duration::from_secs(2), || {
        actor.snapshot().phase == SnapshotPhase::Disconnected
    });
    let disconnected = actor.snapshot();
    assert!(disconnected
        .parameters
        .iter()
        .all(|parameter| !parameter.sync_known));
    assert!(disconnected.active_subscription.is_none());
    assert!(disconnected.desired_subscription.is_some());
    assert!(disconnected.diagnostics.inbound_bytes >= connected_diagnostics.inbound_bytes);
    assert!(disconnected.diagnostics.outbound_bytes >= connected_diagnostics.outbound_bytes);
    assert!(events
        .drain()
        .into_iter()
        .any(|event| matches!(event.payload, CoreEventPayload::ConnectionLost(_))));

    actor.shutdown().unwrap();
}

#[test]
fn consecutive_slider_writes_are_coalesced_before_the_protocol_barrier() {
    let server = SimulatorServer::spawn("127.0.0.1:0".parse().unwrap()).unwrap();
    let actor = AppActorHandle::spawn(
        CoreConfig::simulator(server.local_addr())
            .with_command_batch_window(Duration::from_millis(10)),
    )
    .unwrap();
    let events = actor.subscribe().unwrap();
    actor.send(CoreCommand::Connect).unwrap();
    wait_until(Duration::from_secs(2), || {
        actor.snapshot().phase == SnapshotPhase::Ready
    });
    let initial_revision = actor
        .snapshot()
        .parameters
        .iter()
        .find(|parameter| parameter.param_id == 1)
        .unwrap()
        .revision;
    events.drain();

    let ids = [1.1_f32, 1.2, 1.3].map(|value| {
        actor
            .send(CoreCommand::WriteParameter {
                param_id: 1,
                value: ParamValueDto::F32(value),
            })
            .unwrap()
    });
    wait_until(Duration::from_secs(1), || {
        actor
            .snapshot()
            .parameters
            .iter()
            .find(|parameter| parameter.param_id == 1)
            .is_some_and(|parameter| parameter.ram_value == ParamValueDto::F32(1.3))
    });
    let deadline = Instant::now() + Duration::from_secs(1);
    let mut results = Vec::new();
    while Instant::now() < deadline && results.len() < 3 {
        if let Ok(event) = events.recv_timeout(Duration::from_millis(25)) {
            if let CoreEventPayload::OperationCompleted(result) = event.payload {
                if ids.contains(&result.operation_id) {
                    results.push(result);
                }
            }
        }
    }
    assert_eq!(
        results
            .iter()
            .filter(|result| result.status == OperationStatus::Superseded)
            .count(),
        2
    );
    assert_eq!(
        results
            .iter()
            .filter(|result| result.status == OperationStatus::Succeeded)
            .count(),
        1
    );
    assert_eq!(
        actor
            .snapshot()
            .parameters
            .iter()
            .find(|parameter| parameter.param_id == 1)
            .unwrap()
            .revision,
        initial_revision.wrapping_add(1)
    );

    actor.shutdown().unwrap();
    server.shutdown().unwrap();
}
