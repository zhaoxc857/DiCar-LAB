use dctp_protocol::{
    canonical_parameter_crc32, CapabilityFlags, DeviceManifest, EnumOption, ParamCommitAck,
    ParamConstraints, ParamDescriptor, ParamFlags, ParamState, ParamValue, ParamWriteAck,
    MANIFEST_SCHEMA_VERSION,
};
use dicar_app_core::{
    AccessProfile, AccessRole, CommitFailureKind, ConnectedDevice, ConnectionPhase, DeviceIdentity,
    DeviceSyncState, DiagnosticsSnapshot, LeaseState, ParameterWorkspace, PermissionDecision,
    WorkspaceError, WriteFailure, WriteState,
};

fn descriptor(param_id: u32, value: ParamValue) -> ParamDescriptor {
    ParamDescriptor {
        param_id,
        param_type: value.param_type(),
        flags: ParamFlags::WRITABLE | ParamFlags::PERSISTENT,
        machine_name: format!("p{param_id}"),
        display_name: format!("参数 {param_id}"),
        group: "控制".into(),
        unit: String::new(),
        default_value: value,
        constraints: ParamConstraints::None,
    }
}

fn manifest_and_states() -> (DeviceManifest, Vec<ParamState>) {
    (
        DeviceManifest {
            schema_version: MANIFEST_SCHEMA_VERSION,
            parameters: vec![
                descriptor(1, ParamValue::U32(10)),
                descriptor(2, ParamValue::U32(20)),
            ],
            telemetry: Vec::new(),
        },
        vec![
            ParamState {
                param_id: 1,
                revision: 3,
                value: ParamValue::U32(11),
                persisted_value: Some(ParamValue::U32(10)),
            },
            ParamState {
                param_id: 2,
                revision: 7,
                value: ParamValue::U32(22),
                persisted_value: Some(ParamValue::U32(20)),
            },
        ],
    )
}

#[test]
fn workspace_matches_shuffled_states_by_param_id() {
    let (manifest, mut states) = manifest_and_states();
    states.reverse();

    let workspace = ParameterWorkspace::from_manifest_and_states(&manifest, &states).unwrap();

    assert_eq!(workspace.get(1).unwrap().ram_value, ParamValue::U32(11));
    assert_eq!(workspace.get(1).unwrap().revision, 3);
    assert_eq!(workspace.get(2).unwrap().ram_value, ParamValue::U32(22));
    assert_eq!(workspace.get(2).unwrap().revision, 7);
}

#[test]
fn workspace_rejects_duplicate_missing_unknown_and_invalid_state_records() {
    let (manifest, states) = manifest_and_states();

    let mut duplicate_manifest = manifest.clone();
    duplicate_manifest
        .parameters
        .push(descriptor(1, ParamValue::U32(30)));
    assert_eq!(
        ParameterWorkspace::from_manifest_and_states(&duplicate_manifest, &states).unwrap_err(),
        WorkspaceError::DuplicateDescriptor(1)
    );

    let mut duplicate_states = states.clone();
    duplicate_states.push(states[0].clone());
    assert_eq!(
        ParameterWorkspace::from_manifest_and_states(&manifest, &duplicate_states).unwrap_err(),
        WorkspaceError::DuplicateState(1)
    );

    assert_eq!(
        ParameterWorkspace::from_manifest_and_states(&manifest, &states[..1]).unwrap_err(),
        WorkspaceError::MissingState(2)
    );

    let mut unknown_states = states.clone();
    unknown_states.push(ParamState {
        param_id: 99,
        revision: 0,
        value: ParamValue::U32(1),
        persisted_value: Some(ParamValue::U32(1)),
    });
    assert_eq!(
        ParameterWorkspace::from_manifest_and_states(&manifest, &unknown_states).unwrap_err(),
        WorkspaceError::UnknownState(99)
    );

    let mut wrong_type = states.clone();
    wrong_type[0].value = ParamValue::I32(11);
    assert_eq!(
        ParameterWorkspace::from_manifest_and_states(&manifest, &wrong_type).unwrap_err(),
        WorkspaceError::TypeMismatch(1)
    );

    let mut wrong_persisted_type = states.clone();
    wrong_persisted_type[0].persisted_value = Some(ParamValue::I32(10));
    assert_eq!(
        ParameterWorkspace::from_manifest_and_states(&manifest, &wrong_persisted_type).unwrap_err(),
        WorkspaceError::TypeMismatch(1)
    );

    let mut missing_persisted = states.clone();
    missing_persisted[0].persisted_value = None;
    assert_eq!(
        ParameterWorkspace::from_manifest_and_states(&manifest, &missing_persisted).unwrap_err(),
        WorkspaceError::InvalidPersistence(1)
    );

    let mut non_persistent_manifest = manifest.clone();
    non_persistent_manifest.parameters[0].flags = ParamFlags::WRITABLE;
    assert_eq!(
        ParameterWorkspace::from_manifest_and_states(&non_persistent_manifest, &states)
            .unwrap_err(),
        WorkspaceError::InvalidPersistence(1)
    );
}

#[test]
fn access_policy_requires_role_and_active_local_lease() {
    let cases = [
        (
            AccessProfile::new(AccessRole::Observer, LeaseState::Active),
            PermissionDecision::Denied("仅观察者不能修改参数"),
            PermissionDecision::Denied("当前身份没有固化权限"),
        ),
        (
            AccessProfile::new(AccessRole::Owner, LeaseState::Inactive),
            PermissionDecision::Denied("当前设备没有活动控制租约"),
            PermissionDecision::Denied("当前设备没有活动控制租约"),
        ),
        (
            AccessProfile::new(AccessRole::Tuner, LeaseState::Inactive),
            PermissionDecision::Denied("当前设备没有活动控制租约"),
            PermissionDecision::Denied("当前身份没有固化权限"),
        ),
        (
            AccessProfile::new(AccessRole::Tuner, LeaseState::Active),
            PermissionDecision::Allowed,
            PermissionDecision::Denied("当前身份没有固化权限"),
        ),
        (
            AccessProfile::new(AccessRole::Owner, LeaseState::Active),
            PermissionDecision::Allowed,
            PermissionDecision::Allowed,
        ),
    ];

    for (profile, write, commit) in cases {
        assert_eq!(profile.can_write(), write);
        assert_eq!(profile.can_commit(), commit);
    }
}

#[test]
fn dirty_state_compares_exact_f32_wire_bits() {
    fn f32_workspace(ram_bits: u32, flash_bits: u32) -> ParameterWorkspace {
        let manifest = DeviceManifest {
            schema_version: MANIFEST_SCHEMA_VERSION,
            parameters: vec![descriptor(7, ParamValue::F32(f32::from_bits(flash_bits)))],
            telemetry: Vec::new(),
        };
        ParameterWorkspace::from_manifest_and_states(
            &manifest,
            &[ParamState {
                param_id: 7,
                revision: 1,
                value: ParamValue::F32(f32::from_bits(ram_bits)),
                persisted_value: Some(ParamValue::F32(f32::from_bits(flash_bits))),
            }],
        )
        .unwrap()
    }

    assert!(f32_workspace(0x8000_0000, 0).get(7).unwrap().dirty);
    assert!(
        !f32_workspace(0x7fc0_0001, 0x7fc0_0001)
            .get(7)
            .unwrap()
            .dirty
    );
    assert!(
        f32_workspace(0x7fc0_0002, 0x7fc0_0001)
            .get(7)
            .unwrap()
            .dirty
    );
}

fn single_workspace(mut descriptor: ParamDescriptor) -> ParameterWorkspace {
    descriptor.param_id = 1;
    let value = descriptor.default_value.clone();
    let persisted_value =
        (descriptor.flags.bits() & ParamFlags::PERSISTENT.bits() != 0).then(|| value.clone());
    ParameterWorkspace::from_manifest_and_states(
        &DeviceManifest {
            schema_version: MANIFEST_SCHEMA_VERSION,
            parameters: vec![descriptor],
            telemetry: Vec::new(),
        },
        &[ParamState {
            param_id: 1,
            revision: 9,
            value,
            persisted_value,
        }],
    )
    .unwrap()
}

fn numeric_descriptor(
    default_value: ParamValue,
    min: ParamValue,
    max: ParamValue,
) -> ParamDescriptor {
    let mut value = descriptor(1, default_value);
    value.constraints = ParamConstraints::Numeric {
        min,
        max,
        step: match value.param_type {
            dctp_protocol::ParamType::I32 => ParamValue::I32(3),
            dctp_protocol::ParamType::U32 => ParamValue::U32(3),
            dctp_protocol::ParamType::F32 => ParamValue::F32(0.25),
            _ => unreachable!(),
        },
    };
    value
}

#[test]
fn local_validation_accepts_all_types_and_off_step_values_within_bounds() {
    let owner = AccessProfile::new(AccessRole::Owner, LeaseState::Active);
    let cases = [
        (
            numeric_descriptor(
                ParamValue::I32(0),
                ParamValue::I32(-10),
                ParamValue::I32(10),
            ),
            ParamValue::I32(2),
        ),
        (
            numeric_descriptor(ParamValue::U32(0), ParamValue::U32(0), ParamValue::U32(10)),
            ParamValue::U32(2),
        ),
        (
            numeric_descriptor(
                ParamValue::F32(0.0),
                ParamValue::F32(-1.0),
                ParamValue::F32(1.0),
            ),
            ParamValue::F32(0.3),
        ),
        (
            descriptor(1, ParamValue::Bool(false)),
            ParamValue::Bool(true),
        ),
    ];
    for (descriptor, target) in cases {
        assert!(single_workspace(descriptor)
            .queue_write(owner, 1, target)
            .unwrap()
            .is_some());
    }

    let mut enum_descriptor = descriptor(1, ParamValue::Enum(1));
    enum_descriptor.constraints = ParamConstraints::Enum {
        options: vec![
            EnumOption {
                value: 1,
                label: "一".into(),
            },
            EnumOption {
                value: 2,
                label: "二".into(),
            },
        ],
    };
    assert!(single_workspace(enum_descriptor)
        .queue_write(owner, 1, ParamValue::Enum(2))
        .unwrap()
        .is_some());

    let unconstrained = descriptor(1, ParamValue::F32(0.0));
    let nan = ParamValue::F32(f32::from_bits(0x7fc0_4321));
    assert!(single_workspace(unconstrained.clone())
        .queue_write(owner, 1, nan)
        .unwrap()
        .is_some());
    assert!(single_workspace(unconstrained)
        .queue_write(owner, 1, ParamValue::F32(f32::NEG_INFINITY))
        .unwrap()
        .is_some());

    for boundary in [ParamValue::I32(-10), ParamValue::I32(10)] {
        assert!(single_workspace(numeric_descriptor(
            ParamValue::I32(0),
            ParamValue::I32(-10),
            ParamValue::I32(10),
        ))
        .queue_write(owner, 1, boundary)
        .unwrap()
        .is_some());
    }
}

#[test]
fn local_validation_rejects_type_range_enum_and_read_only_before_queueing() {
    let owner = AccessProfile::new(AccessRole::Owner, LeaseState::Active);
    let numeric = numeric_descriptor(
        ParamValue::F32(0.0),
        ParamValue::F32(-1.0),
        ParamValue::F32(1.0),
    );
    for invalid in [
        ParamValue::F32(-1.01),
        ParamValue::F32(1.01),
        ParamValue::F32(f32::NAN),
        ParamValue::F32(f32::INFINITY),
    ] {
        assert_eq!(
            single_workspace(numeric.clone())
                .queue_write(owner, 1, invalid)
                .unwrap_err(),
            WorkspaceError::OutOfRange(1)
        );
    }
    assert_eq!(
        single_workspace(numeric.clone())
            .queue_write(owner, 1, ParamValue::U32(1))
            .unwrap_err(),
        WorkspaceError::TypeMismatch(1)
    );
    for (descriptor, invalid) in [
        (
            numeric_descriptor(
                ParamValue::I32(0),
                ParamValue::I32(-10),
                ParamValue::I32(10),
            ),
            ParamValue::I32(-11),
        ),
        (
            numeric_descriptor(
                ParamValue::I32(0),
                ParamValue::I32(-10),
                ParamValue::I32(10),
            ),
            ParamValue::I32(11),
        ),
        (
            numeric_descriptor(ParamValue::U32(0), ParamValue::U32(0), ParamValue::U32(10)),
            ParamValue::U32(11),
        ),
    ] {
        assert_eq!(
            single_workspace(descriptor)
                .queue_write(owner, 1, invalid)
                .unwrap_err(),
            WorkspaceError::OutOfRange(1)
        );
    }

    let mut enum_descriptor = descriptor(1, ParamValue::Enum(1));
    enum_descriptor.constraints = ParamConstraints::Enum {
        options: vec![EnumOption {
            value: 1,
            label: "一".into(),
        }],
    };
    assert_eq!(
        single_workspace(enum_descriptor)
            .queue_write(owner, 1, ParamValue::Enum(2))
            .unwrap_err(),
        WorkspaceError::InvalidEnum(1)
    );

    let mut read_only = descriptor(1, ParamValue::Bool(false));
    read_only.flags = ParamFlags::PERSISTENT;
    assert_eq!(
        single_workspace(read_only)
            .queue_write(owner, 1, ParamValue::Bool(true))
            .unwrap_err(),
        WorkspaceError::ReadOnly(1)
    );
}

#[test]
fn denied_access_is_reported_before_parameter_validation() {
    let observer = AccessProfile::new(AccessRole::Observer, LeaseState::Active);
    let error = single_workspace(descriptor(1, ParamValue::U32(1)))
        .queue_write(observer, 1, ParamValue::I32(-1))
        .unwrap_err();
    assert_eq!(
        error,
        WorkspaceError::PermissionDenied("仅观察者不能修改参数")
    );
}

fn owner() -> AccessProfile {
    AccessProfile::new(AccessRole::Owner, LeaseState::Active)
}

#[test]
fn coalescing_keeps_only_latest_target_and_uses_ack_revision() {
    let (manifest, states) = manifest_and_states();
    let mut workspace = ParameterWorkspace::from_manifest_and_states(&manifest, &states).unwrap();

    let first = workspace
        .queue_write(owner(), 1, ParamValue::U32(15))
        .unwrap()
        .unwrap();
    assert_eq!(first.expected_revision, 3);
    assert!(workspace
        .queue_write(owner(), 1, ParamValue::U32(16))
        .unwrap()
        .is_none());
    assert!(workspace
        .queue_write(owner(), 1, ParamValue::U32(17))
        .unwrap()
        .is_none());

    let follow_up = workspace
        .resolve_write(
            1,
            &first,
            Ok(ParamWriteAck {
                value: ParamValue::U32(14),
                new_revision: 4,
            }),
        )
        .unwrap()
        .unwrap();

    let record = workspace.get(1).unwrap();
    assert_eq!(record.ram_value, ParamValue::U32(14));
    assert_eq!(record.revision, 4);
    assert_eq!(record.persisted_value, Some(ParamValue::U32(10)));
    assert!(record.dirty);
    assert_eq!(record.write_state, WriteState::InFlight);
    assert_eq!(follow_up.value, ParamValue::U32(17));
    assert_eq!(follow_up.expected_revision, 4);
}

#[test]
fn queued_value_equal_to_ack_is_not_sent_again_and_other_ids_are_independent() {
    let (manifest, states) = manifest_and_states();
    let mut workspace = ParameterWorkspace::from_manifest_and_states(&manifest, &states).unwrap();
    let first = workspace
        .queue_write(owner(), 1, ParamValue::U32(15))
        .unwrap()
        .unwrap();
    workspace
        .queue_write(owner(), 1, ParamValue::U32(16))
        .unwrap();
    assert!(workspace
        .queue_write(owner(), 2, ParamValue::U32(25))
        .unwrap()
        .is_some());

    assert!(workspace
        .resolve_write(
            1,
            &first,
            Ok(ParamWriteAck {
                value: ParamValue::U32(16),
                new_revision: 4,
            }),
        )
        .unwrap()
        .is_none());
    assert_eq!(workspace.get(1).unwrap().write_state, WriteState::Idle);
    assert_eq!(workspace.get(2).unwrap().write_state, WriteState::InFlight);
}

#[test]
fn ordinary_failure_keeps_confirmed_state_and_clears_latest_queue() {
    let (manifest, states) = manifest_and_states();
    let mut workspace = ParameterWorkspace::from_manifest_and_states(&manifest, &states).unwrap();
    let first = workspace
        .queue_write(owner(), 1, ParamValue::U32(15))
        .unwrap()
        .unwrap();
    workspace
        .queue_write(owner(), 1, ParamValue::U32(16))
        .unwrap();

    assert!(workspace
        .resolve_write(1, &first, Err(WriteFailure::Ordinary))
        .unwrap()
        .is_none());
    let record = workspace.get(1).unwrap();
    assert_eq!(record.ram_value, ParamValue::U32(11));
    assert_eq!(record.revision, 3);
    assert_eq!(record.write_state, WriteState::Idle);
    assert_eq!(record.last_error, Some("参数写入失败，请重试"));
    assert!(workspace
        .queue_write(owner(), 1, ParamValue::U32(17))
        .unwrap()
        .is_some());
}

#[test]
fn revision_conflict_refreshes_device_state_and_preserves_latest_user_target() {
    let (manifest, states) = manifest_and_states();
    let mut workspace = ParameterWorkspace::from_manifest_and_states(&manifest, &states).unwrap();
    let first = workspace
        .queue_write(owner(), 1, ParamValue::U32(15))
        .unwrap()
        .unwrap();
    workspace
        .queue_write(owner(), 1, ParamValue::U32(17))
        .unwrap();

    assert!(workspace
        .resolve_write(
            1,
            &first,
            Err(WriteFailure::RevisionConflict(ParamWriteAck {
                value: ParamValue::U32(99),
                new_revision: 12,
            })),
        )
        .unwrap()
        .is_none());

    let record = workspace.get(1).unwrap();
    assert_eq!(record.ram_value, ParamValue::U32(99));
    assert_eq!(record.revision, 12);
    assert_eq!(record.unresolved_target, Some(ParamValue::U32(17)));
    assert_eq!(record.write_state, WriteState::Idle);
    assert_eq!(record.last_error, Some("参数已在设备端变化，请确认后重试"));
}

fn confirm_write(
    workspace: &mut ParameterWorkspace,
    param_id: u32,
    target: ParamValue,
    accepted: ParamValue,
    revision: u32,
) {
    let pending = workspace
        .queue_write(owner(), param_id, target)
        .unwrap()
        .unwrap();
    assert!(workspace
        .resolve_write(
            param_id,
            &pending,
            Ok(ParamWriteAck {
                value: accepted,
                new_revision: revision,
            }),
        )
        .unwrap()
        .is_none());
}

#[test]
fn commit_plan_is_sorted_uses_exact_value_set_and_applies_matching_ack_atomically() {
    let (manifest, states) = manifest_and_states();
    let mut workspace = ParameterWorkspace::from_manifest_and_states(&manifest, &states).unwrap();
    confirm_write(
        &mut workspace,
        2,
        ParamValue::U32(25),
        ParamValue::U32(24),
        8,
    );
    confirm_write(
        &mut workspace,
        1,
        ParamValue::U32(15),
        ParamValue::U32(14),
        4,
    );

    let plan = workspace.commit_dirty(owner()).unwrap().unwrap();
    assert_eq!(
        plan.entries()
            .iter()
            .map(|entry| (entry.param_id, entry.revision))
            .collect::<Vec<_>>(),
        vec![(1, 4), (2, 8)]
    );
    assert_eq!(
        plan.values(),
        &[(1, ParamValue::U32(14)), (2, ParamValue::U32(24))]
    );
    assert_eq!(
        plan.canonical_crc32(),
        canonical_parameter_crc32(plan.values()).unwrap()
    );

    workspace
        .resolve_commit(
            &plan,
            Ok(ParamCommitAck {
                canonical_crc32: plan.canonical_crc32(),
                storage_generation: 7,
            }),
        )
        .unwrap();
    assert_eq!(
        workspace.get(1).unwrap().persisted_value,
        Some(ParamValue::U32(14))
    );
    assert_eq!(
        workspace.get(2).unwrap().persisted_value,
        Some(ParamValue::U32(24))
    );
    assert_eq!(workspace.dirty_count(), 0);
    assert_eq!(workspace.storage_generation(), 7);
}

#[test]
fn commit_empty_pending_permission_and_all_failures_preserve_persistent_state() {
    let (manifest, mut states) = manifest_and_states();
    for state in &mut states {
        state.value = state.persisted_value.clone().unwrap();
    }
    let mut clean = ParameterWorkspace::from_manifest_and_states(&manifest, &states).unwrap();
    assert!(clean.commit_dirty(owner()).unwrap().is_none());

    let observer = AccessProfile::new(AccessRole::Observer, LeaseState::Active);
    assert_eq!(
        clean.commit_dirty(observer).unwrap_err(),
        WorkspaceError::PermissionDenied("当前身份没有固化权限")
    );

    let mut pending = clean.clone();
    pending
        .queue_write(owner(), 1, ParamValue::U32(15))
        .unwrap();
    assert_eq!(
        pending.commit_dirty(owner()).unwrap_err(),
        WorkspaceError::WritesPending
    );
    pending
        .queue_write(owner(), 1, ParamValue::U32(16))
        .unwrap();
    assert_eq!(
        pending.commit_dirty(owner()).unwrap_err(),
        WorkspaceError::WritesPending
    );

    let mut dirty = clean;
    confirm_write(&mut dirty, 1, ParamValue::U32(15), ParamValue::U32(14), 4);
    for failure in [
        CommitFailureKind::Storage,
        CommitFailureKind::Verify,
        CommitFailureKind::Device,
        CommitFailureKind::Timeout,
    ] {
        let plan = dirty.commit_dirty(owner()).unwrap().unwrap();
        assert_eq!(
            dirty.resolve_commit(&plan, Err(failure)).unwrap_err(),
            WorkspaceError::CommitFailed(failure)
        );
        assert_eq!(
            dirty.get(1).unwrap().persisted_value,
            Some(ParamValue::U32(10))
        );
        assert!(dirty.get(1).unwrap().dirty);
        assert_eq!(dirty.storage_generation(), 0);
    }
    let plan = dirty.commit_dirty(owner()).unwrap().unwrap();
    assert_eq!(
        dirty
            .resolve_commit(
                &plan,
                Ok(ParamCommitAck {
                    canonical_crc32: plan.canonical_crc32() ^ 1,
                    storage_generation: 99,
                }),
            )
            .unwrap_err(),
        WorkspaceError::CommitCrcMismatch
    );
    assert_eq!(
        dirty.get(1).unwrap().persisted_value,
        Some(ParamValue::U32(10))
    );
    assert!(dirty.get(1).unwrap().dirty);
    assert_eq!(dirty.storage_generation(), 0);
}

#[test]
fn second_entry_commit_failure_preserves_both_dirty_records_atomically() {
    let (manifest, states) = manifest_and_states();
    let mut workspace = ParameterWorkspace::from_manifest_and_states(&manifest, &states).unwrap();
    let before_first = workspace.get(1).unwrap().clone();
    let before_second = workspace.get(2).unwrap().clone();
    let plan = workspace.commit_dirty(owner()).unwrap().unwrap();
    assert_eq!(
        plan.entries()
            .iter()
            .map(|entry| entry.param_id)
            .collect::<Vec<_>>(),
        vec![1, 2]
    );

    assert_eq!(
        workspace
            .resolve_commit(&plan, Err(CommitFailureKind::Verify))
            .unwrap_err(),
        WorkspaceError::CommitFailed(CommitFailureKind::Verify)
    );
    assert_eq!(workspace.get(1).unwrap(), &before_first);
    assert_eq!(workspace.get(2).unwrap(), &before_second);
    assert_eq!(workspace.dirty_count(), 2);
    assert_eq!(workspace.storage_generation(), 0);

    let retry = workspace.commit_dirty(owner()).unwrap().unwrap();
    assert_ne!(retry.operation_token(), plan.operation_token());
    assert_eq!(retry.entries(), plan.entries());
}

#[test]
fn active_commit_blocks_writes_and_requires_the_exact_plan_once() {
    let (manifest, states) = manifest_and_states();
    let mut workspace = ParameterWorkspace::from_manifest_and_states(&manifest, &states).unwrap();
    let plan = workspace.commit_dirty(owner()).unwrap().unwrap();

    assert_eq!(
        workspace
            .queue_write(owner(), 1, ParamValue::U32(30))
            .unwrap_err(),
        WorkspaceError::CommitInFlight
    );
    assert_eq!(
        workspace.commit_dirty(owner()).unwrap_err(),
        WorkspaceError::CommitInFlight
    );

    workspace
        .resolve_commit(&plan, Err(CommitFailureKind::Timeout))
        .unwrap_err();
    let replacement = workspace.commit_dirty(owner()).unwrap().unwrap();
    assert_ne!(plan.operation_token(), replacement.operation_token());

    let before = workspace.get(1).unwrap().clone();
    assert_eq!(
        workspace
            .resolve_commit(
                &plan,
                Ok(ParamCommitAck {
                    canonical_crc32: replacement.canonical_crc32(),
                    storage_generation: 8,
                }),
            )
            .unwrap_err(),
        WorkspaceError::CommitOperationMismatch
    );
    assert_eq!(workspace.get(1).unwrap(), &before);

    workspace
        .resolve_commit(
            &replacement,
            Ok(ParamCommitAck {
                canonical_crc32: replacement.canonical_crc32(),
                storage_generation: 8,
            }),
        )
        .unwrap();
    let committed = workspace.get(1).unwrap().clone();
    assert_eq!(
        workspace
            .resolve_commit(
                &replacement,
                Ok(ParamCommitAck {
                    canonical_crc32: replacement.canonical_crc32(),
                    storage_generation: 99,
                }),
            )
            .unwrap_err(),
        WorkspaceError::StaleCommitOperation
    );
    assert_eq!(workspace.get(1).unwrap(), &committed);
}

#[test]
fn revert_uses_persisted_value_and_current_revision_without_optimistic_mutation() {
    let (manifest, states) = manifest_and_states();
    let mut workspace = ParameterWorkspace::from_manifest_and_states(&manifest, &states).unwrap();

    let pending = workspace.revert_parameter(owner(), 1).unwrap();
    assert_eq!(pending.value, ParamValue::U32(10));
    assert_eq!(pending.expected_revision, 3);
    assert_eq!(workspace.get(1).unwrap().ram_value, ParamValue::U32(11));
    assert!(workspace.get(1).unwrap().dirty);

    workspace
        .resolve_write(
            1,
            &pending,
            Ok(ParamWriteAck {
                value: ParamValue::U32(10),
                new_revision: 4,
            }),
        )
        .unwrap();
    assert_eq!(workspace.get(1).unwrap().ram_value, ParamValue::U32(10));
    assert!(!workspace.get(1).unwrap().dirty);
}

#[test]
fn non_persistent_parameter_is_explicitly_not_revertible() {
    let mut descriptor = descriptor(1, ParamValue::U32(10));
    descriptor.flags = ParamFlags::WRITABLE;
    let mut workspace = single_workspace(descriptor);
    assert_eq!(
        workspace.revert_parameter(owner(), 1).unwrap_err(),
        WorkspaceError::NotRevertible(1)
    );
}

#[test]
fn failed_undo_retains_history_and_successful_undo_records_reversible_inverse() {
    let (manifest, mut states) = manifest_and_states();
    states[0].value = ParamValue::U32(10);
    states[1].value = ParamValue::U32(20);
    let mut workspace = ParameterWorkspace::from_manifest_and_states(&manifest, &states).unwrap();
    confirm_write(
        &mut workspace,
        1,
        ParamValue::U32(20),
        ParamValue::U32(20),
        4,
    );
    assert_eq!(workspace.history_snapshot().len(), 1);

    let undo = workspace
        .undo_last_confirmed_change(owner())
        .unwrap()
        .unwrap();
    assert_eq!(undo.value, ParamValue::U32(10));
    assert_eq!(undo.expected_revision, 4);
    workspace
        .resolve_write(1, &undo, Err(WriteFailure::Ordinary))
        .unwrap();
    assert_eq!(workspace.history_snapshot().len(), 1);
    assert_eq!(workspace.get(1).unwrap().ram_value, ParamValue::U32(20));

    let undo = workspace
        .undo_last_confirmed_change(owner())
        .unwrap()
        .unwrap();
    workspace
        .resolve_write(
            1,
            &undo,
            Ok(ParamWriteAck {
                value: undo.value.clone(),
                new_revision: 5,
            }),
        )
        .unwrap();
    assert_eq!(workspace.history_snapshot().len(), 2);
    assert_eq!(workspace.get(1).unwrap().ram_value, ParamValue::U32(10));

    let inverse = workspace
        .undo_last_confirmed_change(owner())
        .unwrap()
        .unwrap();
    assert_eq!(inverse.value, ParamValue::U32(20));
    assert_eq!(inverse.expected_revision, 5);
}

#[test]
fn confirmed_write_history_is_bounded_to_128_and_evicts_oldest() {
    let (manifest, mut states) = manifest_and_states();
    states[0].value = ParamValue::U32(0);
    states[0].persisted_value = Some(ParamValue::U32(0));
    let mut workspace = ParameterWorkspace::from_manifest_and_states(&manifest, &states).unwrap();
    for next in 1..=130 {
        confirm_write(
            &mut workspace,
            1,
            ParamValue::U32(next),
            ParamValue::U32(next),
            next,
        );
    }
    let history = workspace.history_snapshot();
    assert_eq!(history.len(), 128);
    assert_eq!(history[0].previous_value, ParamValue::U32(2));
    assert_eq!(history[127].previous_value, ParamValue::U32(129));
}

#[test]
fn revert_all_reports_partial_failure_and_only_ack_confirmed_device_truth() {
    let (manifest, states) = manifest_and_states();
    let mut workspace = ParameterWorkspace::from_manifest_and_states(&manifest, &states).unwrap();
    let plan = workspace.revert_all(owner()).unwrap();
    assert_eq!(
        plan.writes()
            .iter()
            .map(|write| write.param_id)
            .collect::<Vec<_>>(),
        vec![1, 2]
    );

    let report = workspace
        .resolve_revert_all(
            &plan,
            [
                (
                    plan.writes()[0].clone(),
                    Ok(ParamWriteAck {
                        value: ParamValue::U32(10),
                        new_revision: 4,
                    }),
                ),
                (plan.writes()[1].clone(), Err(WriteFailure::Ordinary)),
            ],
        )
        .unwrap();
    assert_eq!(report.confirmed_ids, vec![1]);
    assert_eq!(report.failed_ids, vec![2]);
    assert_eq!(workspace.get(1).unwrap().ram_value, ParamValue::U32(10));
    assert!(!workspace.get(1).unwrap().dirty);
    assert_eq!(workspace.get(2).unwrap().ram_value, ParamValue::U32(22));
    assert!(workspace.get(2).unwrap().dirty);
}

#[test]
fn batch_owned_write_only_resolves_through_the_exact_revert_plan() {
    let (manifest, states) = manifest_and_states();
    let mut workspace = ParameterWorkspace::from_manifest_and_states(&manifest, &states).unwrap();
    let plan = workspace.revert_all(owner()).unwrap();
    let first = plan.writes()[0].clone();
    let before_first = workspace.get(1).unwrap().clone();
    let before_second = workspace.get(2).unwrap().clone();

    assert_eq!(
        workspace
            .resolve_write(
                1,
                &first,
                Ok(ParamWriteAck {
                    value: ParamValue::U32(10),
                    new_revision: 4,
                }),
            )
            .unwrap_err(),
        WorkspaceError::BatchWriteRequiresBatchResolution
    );
    assert_eq!(workspace.pending_write_count(), 2);
    assert_eq!(workspace.get(1).unwrap(), &before_first);
    assert_eq!(workspace.get(2).unwrap(), &before_second);

    let report = workspace
        .resolve_revert_all(
            &plan,
            [
                (
                    plan.writes()[0].clone(),
                    Ok(ParamWriteAck {
                        value: ParamValue::U32(10),
                        new_revision: 4,
                    }),
                ),
                (
                    plan.writes()[1].clone(),
                    Ok(ParamWriteAck {
                        value: ParamValue::U32(20),
                        new_revision: 8,
                    }),
                ),
            ],
        )
        .unwrap();
    assert_eq!(report.confirmed_ids, vec![1, 2]);
    assert_eq!(workspace.pending_write_count(), 0);
    assert_eq!(workspace.dirty_count(), 0);
}

#[test]
fn revert_all_preflight_failure_registers_nothing() {
    let (mut manifest, states) = manifest_and_states();
    manifest.parameters[1].flags = ParamFlags::PERSISTENT;
    let mut workspace = ParameterWorkspace::from_manifest_and_states(&manifest, &states).unwrap();

    assert_eq!(
        workspace.revert_all(owner()).unwrap_err(),
        WorkspaceError::NotRevertible(2)
    );
    assert_eq!(workspace.pending_write_count(), 0);
    assert!(workspace
        .records()
        .all(|record| record.write_state == WriteState::Idle));
}

#[test]
fn revert_batch_requires_exact_complete_coverage_before_any_result_applies() {
    let (manifest, states) = manifest_and_states();
    let mut workspace = ParameterWorkspace::from_manifest_and_states(&manifest, &states).unwrap();
    let plan = workspace.revert_all(owner()).unwrap();
    let first = plan.writes()[0].clone();
    let second = plan.writes()[1].clone();

    assert_eq!(
        workspace
            .queue_write(owner(), 1, ParamValue::U32(30))
            .unwrap_err(),
        WorkspaceError::RevertInFlight
    );
    assert_eq!(
        workspace.revert_all(owner()).unwrap_err(),
        WorkspaceError::RevertInFlight
    );

    let before_first = workspace.get(1).unwrap().clone();
    let before_second = workspace.get(2).unwrap().clone();
    assert_eq!(
        workspace
            .resolve_revert_all(
                &plan,
                [(
                    first.clone(),
                    Ok(ParamWriteAck {
                        value: ParamValue::U32(10),
                        new_revision: 4,
                    })
                )],
            )
            .unwrap_err(),
        WorkspaceError::RevertCoverageMismatch
    );
    assert_eq!(workspace.get(1).unwrap(), &before_first);
    assert_eq!(workspace.get(2).unwrap(), &before_second);
    assert_eq!(workspace.pending_write_count(), 2);

    assert_eq!(
        workspace
            .resolve_revert_all(
                &plan,
                [
                    (
                        first.clone(),
                        Ok(ParamWriteAck {
                            value: ParamValue::U32(10),
                            new_revision: 4,
                        })
                    ),
                    (first, Err(WriteFailure::Ordinary)),
                ],
            )
            .unwrap_err(),
        WorkspaceError::RevertCoverageMismatch
    );
    assert_eq!(workspace.get(1).unwrap(), &before_first);
    assert_eq!(workspace.get(2).unwrap(), &before_second);

    let report = workspace
        .resolve_revert_all(
            &plan,
            [
                (
                    plan.writes()[0].clone(),
                    Ok(ParamWriteAck {
                        value: ParamValue::U32(10),
                        new_revision: 4,
                    }),
                ),
                (second, Err(WriteFailure::Ordinary)),
            ],
        )
        .unwrap();
    assert_eq!(report.confirmed_ids, vec![1]);
    assert_eq!(report.failed_ids, vec![2]);
}

#[test]
fn disconnect_clears_pending_and_history_then_reconnect_replaces_device_truth() {
    let (manifest, mut states) = manifest_and_states();
    states[0].value = ParamValue::U32(10);
    let mut workspace = ParameterWorkspace::from_manifest_and_states(&manifest, &states).unwrap();
    confirm_write(
        &mut workspace,
        1,
        ParamValue::U32(15),
        ParamValue::U32(15),
        4,
    );
    workspace
        .queue_write(owner(), 1, ParamValue::U32(16))
        .unwrap();
    workspace
        .queue_write(owner(), 1, ParamValue::U32(17))
        .unwrap();
    assert_eq!(workspace.history_snapshot().len(), 1);
    assert_eq!(workspace.pending_write_count(), 2);

    workspace.mark_disconnected();
    assert!(workspace
        .records()
        .all(|record| record.sync_state == DeviceSyncState::Unknown));
    assert_eq!(workspace.pending_write_count(), 0);
    assert!(workspace.history_snapshot().is_empty());
    assert_eq!(
        workspace
            .queue_write(owner(), 1, ParamValue::U32(18))
            .unwrap_err(),
        WorkspaceError::DeviceStateUnknown(1)
    );

    let replacement_manifest = DeviceManifest {
        schema_version: MANIFEST_SCHEMA_VERSION,
        parameters: vec![descriptor(1, ParamValue::U32(50))],
        telemetry: Vec::new(),
    };
    let connected = ConnectedDevice {
        phase: ConnectionPhase::Ready,
        session_id: 900,
        negotiated_max_payload: 1_024,
        identity: DeviceIdentity {
            device_id: [9; 16],
            boot_count: 2,
            firmware_version: [2, 0, 0],
            sdk_version: [1, 0, 0],
            capabilities: CapabilityFlags::PARAMETERS | CapabilityFlags::PERSISTENCE,
        },
        manifest: replacement_manifest,
        parameter_states: vec![ParamState {
            param_id: 1,
            revision: 20,
            value: ParamValue::U32(99),
            persisted_value: Some(ParamValue::U32(50)),
        }],
        diagnostics: DiagnosticsSnapshot::default(),
    };
    workspace.replace_from_connected(&connected).unwrap();

    assert_eq!(workspace.records().count(), 1);
    assert_eq!(workspace.get(1).unwrap().ram_value, ParamValue::U32(99));
    assert_eq!(workspace.get(1).unwrap().revision, 20);
    assert_eq!(workspace.get(1).unwrap().sync_state, DeviceSyncState::Known);
    assert_eq!(workspace.pending_write_count(), 0);
    assert!(workspace.history_snapshot().is_empty());
}

#[test]
fn malformed_ack_type_preserves_confirmed_values_and_clears_pending_state() {
    let (manifest, states) = manifest_and_states();
    let mut workspace = ParameterWorkspace::from_manifest_and_states(&manifest, &states).unwrap();
    let pending = workspace
        .queue_write(owner(), 1, ParamValue::U32(15))
        .unwrap()
        .unwrap();
    workspace
        .queue_write(owner(), 1, ParamValue::U32(16))
        .unwrap();

    assert_eq!(
        workspace
            .resolve_write(
                1,
                &pending,
                Ok(ParamWriteAck {
                    value: ParamValue::I32(15),
                    new_revision: 99,
                }),
            )
            .unwrap_err(),
        WorkspaceError::TypeMismatch(1)
    );
    let record = workspace.get(1).unwrap();
    assert_eq!(record.ram_value, ParamValue::U32(11));
    assert_eq!(record.persisted_value, Some(ParamValue::U32(10)));
    assert_eq!(record.revision, 3);
    assert_eq!(record.write_state, WriteState::Idle);
    assert_eq!(workspace.pending_write_count(), 0);
}

#[test]
fn write_resolution_requires_exact_operation_and_rejects_stale_wrong_id_and_old_generation() {
    let (manifest, states) = manifest_and_states();
    let mut workspace = ParameterWorkspace::from_manifest_and_states(&manifest, &states).unwrap();
    let pending = workspace
        .queue_write(owner(), 1, ParamValue::U32(15))
        .unwrap()
        .unwrap();

    assert_eq!(
        workspace
            .resolve_write(
                2,
                &pending,
                Ok(ParamWriteAck {
                    value: ParamValue::U32(15),
                    new_revision: 4,
                }),
            )
            .unwrap_err(),
        WorkspaceError::WriteOperationMismatch
    );
    assert_eq!(workspace.get(1).unwrap().ram_value, ParamValue::U32(11));
    assert_eq!(workspace.pending_write_count(), 1);

    workspace
        .resolve_write(
            1,
            &pending,
            Ok(ParamWriteAck {
                value: ParamValue::U32(15),
                new_revision: 4,
            }),
        )
        .unwrap();
    let confirmed = workspace.get(1).unwrap().clone();
    assert_eq!(
        workspace
            .resolve_write(
                1,
                &pending,
                Ok(ParamWriteAck {
                    value: ParamValue::U32(99),
                    new_revision: 99,
                }),
            )
            .unwrap_err(),
        WorkspaceError::StaleWriteOperation
    );
    assert_eq!(workspace.get(1).unwrap(), &confirmed);

    let old_generation = workspace
        .queue_write(owner(), 1, ParamValue::U32(16))
        .unwrap()
        .unwrap();
    workspace.mark_disconnected();
    let connected = ConnectedDevice {
        phase: ConnectionPhase::Ready,
        session_id: 901,
        negotiated_max_payload: 1_024,
        identity: DeviceIdentity {
            device_id: [7; 16],
            boot_count: 3,
            firmware_version: [3, 0, 0],
            sdk_version: [1, 0, 0],
            capabilities: CapabilityFlags::PARAMETERS | CapabilityFlags::PERSISTENCE,
        },
        manifest: manifest.clone(),
        parameter_states: states.clone(),
        diagnostics: DiagnosticsSnapshot::default(),
    };
    workspace.replace_from_connected(&connected).unwrap();
    let replacement = workspace.get(1).unwrap().clone();
    assert_eq!(
        workspace
            .resolve_write(
                1,
                &old_generation,
                Ok(ParamWriteAck {
                    value: ParamValue::U32(77),
                    new_revision: 77,
                }),
            )
            .unwrap_err(),
        WorkspaceError::OldWorkspaceGeneration
    );
    assert_eq!(workspace.get(1).unwrap(), &replacement);

    let after_reconnect = workspace
        .queue_write(owner(), 1, ParamValue::U32(17))
        .unwrap()
        .unwrap();
    assert_ne!(
        old_generation.operation_token(),
        after_reconnect.operation_token()
    );
    assert_ne!(
        old_generation.workspace_generation(),
        after_reconnect.workspace_generation()
    );
}
