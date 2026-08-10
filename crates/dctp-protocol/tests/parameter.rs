use dctp_protocol::{
    canonical_parameter_crc32, EnumOption, ParamCommit, ParamCommitAck, ParamCommitEntry,
    ParamConstraints, ParamDescriptor, ParamFlags, ParamRead, ParamState, ParamType, ParamValue,
    ParamWrite, ParamWriteAck, ProtocolError, WireDecode, WireEncode,
};

#[test]
fn typed_write_round_trips_with_expected_revision() {
    let write = ParamWrite {
        param_id: 42,
        expected_revision: 7,
        value: ParamValue::F32(1.25),
    };

    assert_eq!(ParamWrite::decode(&write.encode().unwrap()).unwrap(), write);
}

#[test]
fn parameter_state_round_trips_ram_and_flash_values() {
    let state = ParamState {
        param_id: 7,
        revision: 9,
        value: ParamValue::F32(1.25),
        persisted_value: Some(ParamValue::F32(1.0)),
    };

    assert_eq!(ParamState::decode(&state.encode().unwrap()).unwrap(), state);
}

#[test]
fn non_persistent_state_round_trips_without_a_flash_value() {
    let state = ParamState {
        param_id: 8,
        revision: 0,
        value: ParamValue::Bool(true),
        persisted_value: None,
    };

    assert_eq!(ParamState::decode(&state.encode().unwrap()).unwrap(), state);
}

#[test]
fn commit_ack_round_trips_crc_and_generation() {
    let ack = ParamCommitAck {
        canonical_crc32: 0x1234_5678,
        storage_generation: 42,
    };

    assert_eq!(ParamCommitAck::decode(&ack.encode().unwrap()).unwrap(), ack);
}

#[test]
fn wire_equality_distinguishes_signed_zero_and_nan_payload_bits() {
    assert!(!ParamValue::F32(-0.0).wire_eq(&ParamValue::F32(0.0)));
    let first = ParamValue::F32(f32::from_bits(0x7fc0_0001));
    let same = ParamValue::F32(f32::from_bits(0x7fc0_0001));
    let other = ParamValue::F32(f32::from_bits(0x7fc0_0002));
    assert!(first.wire_eq(&same));
    assert!(!first.wire_eq(&other));
}

#[test]
fn parameter_state_rejects_invalid_marker_type_mismatch_and_trailing_bytes() {
    let invalid_marker = [7, 0, 0, 0, 9, 0, 0, 0, 3, 0, 0, 160, 63, 2];
    let type_mismatch = [7, 0, 0, 0, 9, 0, 0, 0, 3, 0, 0, 160, 63, 1, 2, 1, 0, 0, 0];
    let trailing_byte = [8, 0, 0, 0, 0, 0, 0, 0, 4, 1, 0, 0];

    for bytes in [
        invalid_marker.as_slice(),
        type_mismatch.as_slice(),
        trailing_byte.as_slice(),
    ] {
        assert!(ParamState::decode(bytes).is_err());
    }
}

#[test]
fn canonical_crc_is_independent_of_input_order() {
    let a = vec![(20, ParamValue::U32(4)), (10, ParamValue::I32(-2))];
    let b = vec![(10, ParamValue::I32(-2)), (20, ParamValue::U32(4))];

    assert_eq!(canonical_parameter_crc32(&a), canonical_parameter_crc32(&b));
}

#[test]
fn descriptor_rejects_a_machine_name_over_48_bytes() {
    let descriptor = ParamDescriptor {
        param_id: 1,
        param_type: ParamType::U32,
        flags: ParamFlags::WRITABLE,
        machine_name: "m".repeat(49),
        display_name: "Display".into(),
        group: "Group".into(),
        unit: "unit".into(),
        default_value: ParamValue::U32(1),
        constraints: ParamConstraints::None,
    };

    assert!(matches!(
        descriptor.encode(),
        Err(ProtocolError::StringTooLong)
    ));
}

#[test]
fn canonical_crc_rejects_duplicate_parameter_ids() {
    let entries = vec![(7, ParamValue::U32(1)), (7, ParamValue::U32(2))];

    assert!(matches!(
        canonical_parameter_crc32(&entries),
        Err(ProtocolError::InvalidValue)
    ));
}

#[test]
fn negative_zero_f32_retains_its_sign_bit() {
    let write = ParamWrite {
        param_id: 42,
        expected_revision: 7,
        value: ParamValue::F32(-0.0),
    };

    let decoded = ParamWrite::decode(&write.encode().unwrap()).unwrap();
    let ParamValue::F32(value) = decoded.value else {
        panic!("expected f32");
    };
    assert_eq!(value.to_bits(), (-0.0f32).to_bits());
}

#[test]
fn distinct_nan_payloads_remain_distinct_on_the_wire() {
    let first = ParamWrite {
        param_id: 42,
        expected_revision: 7,
        value: ParamValue::F32(f32::from_bits(0x7FC0_0001)),
    };
    let second = ParamWrite {
        param_id: 42,
        expected_revision: 7,
        value: ParamValue::F32(f32::from_bits(0x7FC0_0002)),
    };

    assert_ne!(first.encode().unwrap(), second.encode().unwrap());
}

#[test]
fn bool_values_reject_noncanonical_wire_bytes() {
    let bytes = [1, 0, 0, 0, 0, 0, 0, 0, 4, 2];

    assert!(matches!(
        ParamWrite::decode(&bytes),
        Err(ProtocolError::InvalidValue)
    ));
}

#[test]
fn descriptor_rejects_mismatched_numeric_constraints() {
    let descriptor = ParamDescriptor {
        param_id: 1,
        param_type: ParamType::U32,
        flags: ParamFlags::WRITABLE,
        machine_name: "speed".into(),
        display_name: "Speed".into(),
        group: "Drive".into(),
        unit: "rpm".into(),
        default_value: ParamValue::U32(1),
        constraints: ParamConstraints::Numeric {
            min: ParamValue::I32(0),
            max: ParamValue::U32(10),
            step: ParamValue::U32(1),
        },
    };

    assert!(matches!(
        descriptor.encode(),
        Err(ProtocolError::InvalidValue)
    ));
}

#[test]
fn descriptor_rejects_duplicate_enum_values() {
    let descriptor = ParamDescriptor {
        param_id: 1,
        param_type: ParamType::Enum,
        flags: ParamFlags::WRITABLE,
        machine_name: "mode".into(),
        display_name: "Mode".into(),
        group: "Drive".into(),
        unit: "".into(),
        default_value: ParamValue::Enum(1),
        constraints: ParamConstraints::Enum {
            options: vec![
                EnumOption {
                    value: 1,
                    label: "One".into(),
                },
                EnumOption {
                    value: 1,
                    label: "Duplicate".into(),
                },
            ],
        },
    };

    assert!(matches!(
        descriptor.encode(),
        Err(ProtocolError::InvalidValue)
    ));
}

#[test]
fn commit_requires_entries_sorted_by_parameter_id() {
    let commit = ParamCommit {
        entries: vec![
            ParamCommitEntry {
                param_id: 2,
                revision: 1,
            },
            ParamCommitEntry {
                param_id: 1,
                revision: 1,
            },
        ],
        canonical_crc32: 0,
    };

    assert!(matches!(commit.encode(), Err(ProtocolError::InvalidValue)));
}

#[test]
fn descriptor_and_all_parameter_payloads_round_trip() {
    let descriptor = ParamDescriptor {
        param_id: 3,
        param_type: ParamType::Enum,
        flags: ParamFlags::WRITABLE | ParamFlags::PERSISTENT,
        machine_name: "drive.mode".into(),
        display_name: "Drive mode".into(),
        group: "Drive".into(),
        unit: "".into(),
        default_value: ParamValue::Enum(2),
        constraints: ParamConstraints::Enum {
            options: vec![
                EnumOption {
                    value: 1,
                    label: "Eco".into(),
                },
                EnumOption {
                    value: 2,
                    label: "Sport".into(),
                },
            ],
        },
    };
    assert_eq!(
        ParamDescriptor::decode(&descriptor.encode().unwrap()).unwrap(),
        descriptor
    );

    let read = ParamRead { param_id: 3 };
    assert_eq!(ParamRead::decode(&read.encode().unwrap()).unwrap(), read);

    let state = ParamState {
        param_id: 3,
        revision: 9,
        value: ParamValue::Enum(2),
        persisted_value: None,
    };
    assert_eq!(ParamState::decode(&state.encode().unwrap()).unwrap(), state);

    let ack = ParamWriteAck {
        value: ParamValue::Enum(2),
        new_revision: 9,
    };
    assert_eq!(ParamWriteAck::decode(&ack.encode().unwrap()).unwrap(), ack);

    let commit = ParamCommit {
        entries: vec![
            ParamCommitEntry {
                param_id: 3,
                revision: 9,
            },
            ParamCommitEntry {
                param_id: 10,
                revision: 4,
            },
        ],
        canonical_crc32: 0xAABB_CCDD,
    };
    assert_eq!(
        ParamCommit::decode(&commit.encode().unwrap()).unwrap(),
        commit
    );
}
