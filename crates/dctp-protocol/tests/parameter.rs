use dctp_protocol::{
    canonical_parameter_crc32, EnumOption, ParamCommit, ParamCommitEntry, ParamConstraints,
    ParamDescriptor, ParamFlags, ParamRead, ParamState, ParamType, ParamValue, ParamWrite,
    ParamWriteAck, ProtocolError, WireDecode, WireEncode,
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
