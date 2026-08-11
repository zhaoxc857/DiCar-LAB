use std::time::Duration;

use dctp_protocol::{
    TelemetryBatch, TelemetryDescriptor, TelemetrySample, TelemetrySubscription, TelemetryType,
};
use dicar_app_core::{TelemetryEngine, TelemetryError, TelemetryValue};

fn descriptors() -> Vec<TelemetryDescriptor> {
    [
        (200, TelemetryType::F32, "m/s"),
        (201, TelemetryType::I32, "tick"),
        (202, TelemetryType::U32, "count"),
        (203, TelemetryType::Flags32, ""),
    ]
    .into_iter()
    .map(|(channel_id, telemetry_type, unit)| TelemetryDescriptor {
        channel_id,
        telemetry_type,
        machine_name: format!("test.channel_{channel_id}"),
        display_name: format!("通道 {channel_id}"),
        group: "测试".into(),
        unit: unit.into(),
    })
    .collect()
}

fn subscription(version: u16) -> TelemetrySubscription {
    TelemetrySubscription {
        subscription_version: version,
        sample_rate_hz: 500,
        channel_ids: vec![200, 201, 202, 203],
    }
}

fn batch(version: u16, sequence: u16, dropped: u16, base: u32) -> TelemetryBatch {
    TelemetryBatch {
        subscription_version: version,
        first_sample_sequence: sequence,
        dropped_samples: dropped,
        base_timestamp_us: base,
        samples: vec![TelemetrySample {
            dt_us: 0,
            values: vec![1.25_f32.to_bits(), (-7_i32) as u32, 42, 0xA5A5_0001],
        }],
    }
}

fn engine(version: u16) -> TelemetryEngine {
    let mut engine = TelemetryEngine::new(Duration::from_secs(60), 8);
    engine
        .activate(subscription(version), &descriptors())
        .unwrap();
    engine
}

#[test]
fn converts_all_wire_slots_without_collapsing_integer_or_flag_types() {
    let mut engine = engine(7);
    let accepted = engine.accept(batch(7, 10, 0, 1_000)).unwrap();

    assert_eq!(accepted.points.len(), 4);
    assert_eq!(accepted.points[0].value, TelemetryValue::F32(1.25));
    assert_eq!(accepted.points[1].value, TelemetryValue::I32(-7));
    assert_eq!(accepted.points[2].value, TelemetryValue::U32(42));
    assert_eq!(
        accepted.points[3].value,
        TelemetryValue::Flags32(0xA5A5_0001)
    );
}

#[test]
fn unwraps_u32_time_and_accumulates_sample_deltas() {
    let mut engine = engine(1);
    engine.accept(batch(1, 1, 0, u32::MAX - 1_000)).unwrap();
    let mut after_wrap = batch(1, 2, 0, 750);
    after_wrap.samples.push(TelemetrySample {
        dt_us: 2_000,
        values: vec![0; 4],
    });
    let accepted = engine.accept(after_wrap).unwrap();

    let first = accepted.points[0].timestamp_us;
    let second = accepted.points[4].timestamp_us;
    assert!(first > u64::from(u32::MAX));
    assert_eq!(second - first, 2_000);
    assert_eq!(engine.latest_timestamp_us(), Some(second));
}

#[test]
fn sequence_gap_and_device_drop_counters_remain_distinct_across_wrap() {
    let mut engine = engine(3);
    engine.accept(batch(3, u16::MAX, 0, 1_000)).unwrap();
    engine.accept(batch(3, 3, 2, 3_000)).unwrap();

    let diagnostics = engine.diagnostics();
    assert_eq!(diagnostics.sequence_gap_samples, 3);
    assert_eq!(diagnostics.device_dropped_samples, 2);
}

#[test]
fn rejects_wrong_version_or_width_without_partially_mutating_buffers() {
    let mut engine = engine(9);
    engine.accept(batch(9, 1, 0, 1_000)).unwrap();
    let before = engine.total_points();

    assert_eq!(
        engine.accept(batch(8, 2, 0, 3_000)),
        Err(TelemetryError::StaleSubscription {
            expected: 9,
            actual: 8,
        })
    );
    let mut wrong_width = batch(9, 2, 0, 3_000);
    wrong_width.samples[0].values.pop();
    assert_eq!(
        engine.accept(wrong_width),
        Err(TelemetryError::ChannelWidth {
            expected: 4,
            actual: 3,
        })
    );
    assert_eq!(engine.total_points(), before);
    assert_eq!(engine.diagnostics().rejected_batches, 2);
}

#[test]
fn evicts_oldest_points_at_the_sixty_second_eight_channel_bound() {
    let mut engine = TelemetryEngine::new(Duration::from_secs(60), 8);
    let mut all_descriptors = Vec::new();
    for id in 200..208 {
        all_descriptors.push(TelemetryDescriptor {
            channel_id: id,
            telemetry_type: TelemetryType::U32,
            machine_name: format!("channel.{id}"),
            display_name: format!("通道 {id}"),
            group: "压力".into(),
            unit: "raw".into(),
        });
    }
    engine
        .activate(
            TelemetrySubscription {
                subscription_version: 11,
                sample_rate_hz: 500,
                channel_ids: (200..208).collect(),
            },
            &all_descriptors,
        )
        .unwrap();

    for sequence in 0..30_001_u32 {
        engine
            .accept(TelemetryBatch {
                subscription_version: 11,
                first_sample_sequence: sequence as u16,
                dropped_samples: 0,
                base_timestamp_us: sequence * 2_000,
                samples: vec![TelemetrySample {
                    dt_us: 0,
                    values: vec![sequence; 8],
                }],
            })
            .unwrap();
    }

    assert_eq!(engine.total_points(), 240_000);
    for id in 200..208 {
        assert_eq!(engine.channel_len(id), 30_000);
    }
}
