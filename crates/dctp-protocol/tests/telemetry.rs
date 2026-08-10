use dctp_protocol::{
    DeviceManifest, LogMessage, LogSeverity, ParamConstraints, ParamDescriptor, ParamFlags,
    ParamType, ParamValue, ProtocolError, TelemetryBatch, TelemetryDescriptor, TelemetrySample,
    TelemetrySubscription, TelemetryType, WireDecode, WireEncode,
};

fn telemetry_descriptor(channel_id: u32) -> TelemetryDescriptor {
    TelemetryDescriptor {
        channel_id,
        telemetry_type: TelemetryType::F32,
        machine_name: format!("channel.{channel_id}"),
        display_name: format!("Channel {channel_id}"),
        group: "Drive".into(),
        unit: "rpm".into(),
    }
}

fn parameter_descriptor(param_id: u32) -> ParamDescriptor {
    ParamDescriptor {
        param_id,
        param_type: ParamType::U32,
        flags: ParamFlags::WRITABLE,
        machine_name: format!("parameter.{param_id}"),
        display_name: format!("Parameter {param_id}"),
        group: "Drive".into(),
        unit: "rpm".into(),
        default_value: ParamValue::U32(param_id),
        constraints: ParamConstraints::None,
    }
}

#[test]
fn mixed_telemetry_batch_round_trips() {
    let batch = TelemetryBatch {
        subscription_version: 3,
        first_sample_sequence: 99,
        dropped_samples: 2,
        base_timestamp_us: 1_000_000,
        samples: vec![
            TelemetrySample {
                dt_us: 0,
                values: vec![1.5f32.to_bits(), (-4i32) as u32, 8, 0b101],
            },
            TelemetrySample {
                dt_us: 2_000,
                values: vec![1.75f32.to_bits(), (-3i32) as u32, 9, 0b001],
            },
        ],
    };

    let bytes = batch.encode().unwrap();

    assert_eq!(TelemetryBatch::decode(&bytes, 4).unwrap(), batch);
}

#[test]
fn telemetry_batch_supports_eight_channels_and_sixteen_samples() {
    let batch = TelemetryBatch {
        subscription_version: 1,
        first_sample_sequence: 2,
        dropped_samples: 0,
        base_timestamp_us: 3,
        samples: (0..16)
            .map(|index| TelemetrySample {
                dt_us: if index == 0 { 0 } else { 2_000 },
                values: vec![index; 8],
            })
            .collect(),
    };

    assert_eq!(
        TelemetryBatch::decode(&batch.encode().unwrap(), 8).unwrap(),
        batch
    );
}

#[test]
fn telemetry_batch_rejects_nine_channels() {
    let batch = TelemetryBatch {
        subscription_version: 1,
        first_sample_sequence: 2,
        dropped_samples: 0,
        base_timestamp_us: 3,
        samples: vec![TelemetrySample {
            dt_us: 0,
            values: vec![0; 9],
        }],
    };

    assert!(matches!(batch.encode(), Err(ProtocolError::InvalidValue)));
}

#[test]
fn telemetry_batch_rejects_seventeen_samples() {
    let batch = TelemetryBatch {
        subscription_version: 1,
        first_sample_sequence: 2,
        dropped_samples: 0,
        base_timestamp_us: 3,
        samples: (0..17)
            .map(|index| TelemetrySample {
                dt_us: if index == 0 { 0 } else { 1 },
                values: vec![0],
            })
            .collect(),
    };

    assert!(matches!(batch.encode(), Err(ProtocolError::InvalidValue)));
}

#[test]
fn telemetry_batch_rejects_inconsistent_sample_width() {
    let batch = TelemetryBatch {
        subscription_version: 1,
        first_sample_sequence: 2,
        dropped_samples: 0,
        base_timestamp_us: 3,
        samples: vec![
            TelemetrySample {
                dt_us: 0,
                values: vec![1, 2],
            },
            TelemetrySample {
                dt_us: 1,
                values: vec![3],
            },
        ],
    };

    assert!(matches!(batch.encode(), Err(ProtocolError::InvalidValue)));
}

#[test]
fn telemetry_batch_rejects_nonzero_first_delta_and_trailing_bytes() {
    let batch = TelemetryBatch {
        subscription_version: 1,
        first_sample_sequence: 2,
        dropped_samples: 0,
        base_timestamp_us: 3,
        samples: vec![TelemetrySample {
            dt_us: 1,
            values: vec![0],
        }],
    };
    assert!(matches!(batch.encode(), Err(ProtocolError::InvalidValue)));

    let valid = TelemetryBatch {
        samples: vec![TelemetrySample {
            dt_us: 0,
            values: vec![0],
        }],
        ..batch
    };
    let mut bytes = valid.encode().unwrap();
    bytes.push(0);
    assert!(matches!(
        TelemetryBatch::decode(&bytes, 1),
        Err(ProtocolError::InvalidLength)
    ));
}

#[test]
fn telemetry_descriptor_and_subscription_round_trip_with_unique_ids() {
    let descriptor = TelemetryDescriptor {
        telemetry_type: TelemetryType::Flags32,
        ..telemetry_descriptor(5)
    };
    assert_eq!(
        TelemetryDescriptor::decode(&descriptor.encode().unwrap()).unwrap(),
        descriptor
    );

    let subscription = TelemetrySubscription {
        subscription_version: 7,
        sample_rate_hz: 500,
        channel_ids: vec![5, 6],
    };
    assert_eq!(
        TelemetrySubscription::decode(&subscription.encode().unwrap()).unwrap(),
        subscription
    );
}

#[test]
fn telemetry_subscription_rejects_duplicate_ids_and_rate_over_limit() {
    let duplicate = TelemetrySubscription {
        subscription_version: 1,
        sample_rate_hz: 500,
        channel_ids: vec![2, 2],
    };
    assert!(matches!(
        duplicate.encode(),
        Err(ProtocolError::InvalidValue)
    ));

    let over_rate = TelemetrySubscription {
        subscription_version: 1,
        sample_rate_hz: 501,
        channel_ids: vec![2],
    };
    assert!(matches!(
        over_rate.encode(),
        Err(ProtocolError::InvalidValue)
    ));

    let zero_rate = TelemetrySubscription {
        subscription_version: 1,
        sample_rate_hz: 0,
        channel_ids: vec![2],
    };
    assert!(matches!(
        zero_rate.encode(),
        Err(ProtocolError::InvalidValue)
    ));
}

#[test]
fn log_round_trips_and_rejects_overlong_text() {
    let log = LogMessage {
        timestamp_us: 1_000,
        severity: LogSeverity::Warn,
        module_id: 12,
        text: "controller saturated".into(),
    };
    assert_eq!(LogMessage::decode(&log.encode().unwrap()).unwrap(), log);

    let overlong = LogMessage {
        text: "x".repeat(193),
        ..log
    };
    assert!(matches!(
        overlong.encode(),
        Err(ProtocolError::StringTooLong)
    ));
}

#[test]
fn log_rejects_unknown_severity_and_invalid_utf8() {
    assert!(matches!(
        LogMessage::decode(&[0, 0, 0, 0, 5, 0, 0, 0]),
        Err(ProtocolError::InvalidValue)
    ));
    assert!(matches!(
        LogMessage::decode(&[0, 0, 0, 0, 2, 0, 0, 1, 0xff]),
        Err(ProtocolError::InvalidUtf8)
    ));
}

#[test]
fn manifest_rejects_duplicate_ids() {
    let manifest = DeviceManifest {
        schema_version: 1,
        parameters: vec![parameter_descriptor(4), parameter_descriptor(4)],
        telemetry: vec![],
    };

    assert!(matches!(
        manifest.encode_canonical(),
        Err(ProtocolError::InvalidValue)
    ));
}

#[test]
fn manifest_encoding_sorts_input_and_crc_changes_with_descriptor() {
    let manifest = DeviceManifest {
        schema_version: 1,
        parameters: vec![parameter_descriptor(9), parameter_descriptor(3)],
        telemetry: vec![telemetry_descriptor(8), telemetry_descriptor(2)],
    };
    let encoded = manifest.encode_canonical().unwrap();
    let decoded = DeviceManifest::decode(&encoded).unwrap();
    assert_eq!(
        decoded
            .parameters
            .iter()
            .map(|descriptor| descriptor.param_id)
            .collect::<Vec<_>>(),
        vec![3, 9]
    );
    assert_eq!(
        decoded
            .telemetry
            .iter()
            .map(|descriptor| descriptor.channel_id)
            .collect::<Vec<_>>(),
        vec![2, 8]
    );

    let mut changed = manifest.clone();
    changed.telemetry[0].unit = "rad/s".into();
    assert_ne!(
        manifest.manifest_crc32().unwrap(),
        changed.manifest_crc32().unwrap()
    );
}

#[test]
fn manifest_rejects_a_record_with_trailing_bytes() {
    let manifest = DeviceManifest {
        schema_version: 1,
        parameters: vec![parameter_descriptor(1)],
        telemetry: vec![],
    };
    let mut bytes = manifest.encode_canonical().unwrap();
    let record_len = u16::from_le_bytes([bytes[6], bytes[7]]);
    bytes[6..8].copy_from_slice(&(record_len + 1).to_le_bytes());
    bytes.push(0);

    assert!(matches!(
        DeviceManifest::decode(&bytes),
        Err(ProtocolError::InvalidLength)
    ));
}
