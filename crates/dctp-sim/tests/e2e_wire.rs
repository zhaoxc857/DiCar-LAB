use dctp_protocol::{
    canonical_parameter_crc32, encode_frame, DeviceManifest, ErrorCode, ErrorPayload, Frame,
    FrameFlags, Heartbeat, Hello, HelloAck, LogMessage, LogSeverity, ManifestAssembler,
    ManifestChunk, ManifestDone, MessageType, ParamCommit, ParamCommitAck, ParamCommitEntry,
    ParamRead, ParamState, ParamValue, ParamWrite, ParamWriteAck, ProtocolError, StreamDecoder,
    TelemetryBatch, TelemetrySubscription, WireDecode, WireEncode,
};
use dctp_sim::{CommitFailure, Priority, PriorityTxQueue, PushOutcome, SimConfig, SimDevice};

struct WireHarness {
    device: SimDevice,
    device_decoder: StreamDecoder,
    host_decoder: StreamDecoder,
    tx_queue: PriorityTxQueue,
    next_sequence: u16,
    now_ms: u64,
    corrupt_next_device_packet: Option<(usize, u8)>,
    telemetry_channel_count: usize,
}

impl WireHarness {
    fn new() -> Self {
        Self::with_queue_capacities([8, 32, 16, 16])
    }

    fn with_queue_capacities(capacities: [usize; 4]) -> Self {
        Self {
            device: SimDevice::new(SimConfig::default()),
            device_decoder: StreamDecoder::new(),
            host_decoder: StreamDecoder::new(),
            tx_queue: PriorityTxQueue::with_capacities(capacities),
            next_sequence: 1,
            now_ms: 0,
            corrupt_next_device_packet: None,
            telemetry_channel_count: 0,
        }
    }

    fn hello(&mut self, client_nonce: u32) -> Result<u32, ProtocolError> {
        let payload = Hello {
            client_nonce,
            min_version: 1,
            max_version: 1,
            max_payload: 1_024,
        }
        .encode()?;
        let response = self.only_response(MessageType::Hello, 0, payload)?;
        if response.header.message_type != MessageType::HelloAck {
            return Err(response_error(&response));
        }
        Ok(HelloAck::decode(&response.payload)?.session_id)
    }

    fn heartbeat(&mut self, session_id: u32) -> Result<(), ProtocolError> {
        let payload = Heartbeat {
            monotonic_ms: self.now_ms as u32,
        }
        .encode()?;
        let response = self.only_response(MessageType::Heartbeat, session_id, payload)?;
        if response.header.message_type == MessageType::HeartbeatAck {
            Ok(())
        } else {
            Err(response_error(&response))
        }
    }

    fn manifest(&mut self, session_id: u32) -> Result<DeviceManifest, ProtocolError> {
        let responses = self.exchange(MessageType::ManifestRequest, session_id, Vec::new())?;
        let mut assembler = ManifestAssembler::new();
        let mut done = None;
        for response in responses {
            match response.header.message_type {
                MessageType::ManifestChunk => {
                    assembler.push_chunk(ManifestChunk::decode(&response.payload)?)?
                }
                MessageType::ManifestDone => done = Some(ManifestDone::decode(&response.payload)?),
                _ => return Err(response_error(&response)),
            }
        }
        DeviceManifest::decode(&assembler.finish(done.ok_or(ProtocolError::Truncated)?)?)
    }

    fn read_parameter(
        &mut self,
        session_id: u32,
        param_id: u32,
    ) -> Result<ParamState, ProtocolError> {
        let response = self.only_response(
            MessageType::ParamRead,
            session_id,
            ParamRead { param_id }.encode()?,
        )?;
        if response.header.message_type == MessageType::ParamValue {
            ParamState::decode(&response.payload)
        } else {
            Err(response_error(&response))
        }
    }

    fn write_f32(
        &mut self,
        session_id: u32,
        param_id: u32,
        expected_revision: u32,
        value: f32,
    ) -> Result<ParamWriteAck, ProtocolError> {
        let sequence = self.next_sequence();
        self.write_f32_with_sequence(session_id, param_id, expected_revision, value, sequence)
    }

    fn write_f32_with_sequence(
        &mut self,
        session_id: u32,
        param_id: u32,
        expected_revision: u32,
        value: f32,
        sequence: u16,
    ) -> Result<ParamWriteAck, ProtocolError> {
        let payload = ParamWrite {
            param_id,
            expected_revision,
            value: ParamValue::F32(value),
        }
        .encode()?;
        let response = self.only_response_with_sequence(
            MessageType::ParamWrite,
            session_id,
            payload,
            sequence,
        )?;
        if response.header.message_type == MessageType::ParamWriteAck {
            ParamWriteAck::decode(&response.payload)
        } else {
            Err(response_error(&response))
        }
    }

    fn commit_with_sequence(
        &mut self,
        session_id: u32,
        entries: Vec<(u32, u32)>,
        sequence: u16,
    ) -> Result<ParamCommitAck, ProtocolError> {
        let values = entries
            .iter()
            .map(|(param_id, _)| {
                self.read_parameter(session_id, *param_id)
                    .map(|state| (*param_id, state.value))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let payload = ParamCommit {
            entries: entries
                .into_iter()
                .map(|(param_id, revision)| ParamCommitEntry { param_id, revision })
                .collect(),
            canonical_crc32: canonical_parameter_crc32(&values)?,
        }
        .encode()?;
        let response = self.only_response_with_sequence(
            MessageType::ParamCommit,
            session_id,
            payload,
            sequence,
        )?;
        if response.header.message_type == MessageType::ParamCommitAck {
            ParamCommitAck::decode(&response.payload)
        } else {
            Err(response_error(&response))
        }
    }

    fn commit_response_with_sequence(
        &mut self,
        session_id: u32,
        entries: Vec<(u32, u32)>,
        sequence: u16,
    ) -> Result<Frame, ProtocolError> {
        let values = entries
            .iter()
            .map(|(param_id, _)| {
                self.read_parameter(session_id, *param_id)
                    .map(|state| (*param_id, state.value))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let payload = ParamCommit {
            entries: entries
                .into_iter()
                .map(|(param_id, revision)| ParamCommitEntry { param_id, revision })
                .collect(),
            canonical_crc32: canonical_parameter_crc32(&values)?,
        }
        .encode()?;
        self.only_response_with_sequence(MessageType::ParamCommit, session_id, payload, sequence)
    }

    fn subscribe(&mut self, session_id: u32, channel_ids: Vec<u32>) -> Result<(), ProtocolError> {
        self.subscribe_at(session_id, 100, channel_ids)
    }

    fn subscribe_at(
        &mut self,
        session_id: u32,
        sample_rate_hz: u16,
        channel_ids: Vec<u32>,
    ) -> Result<(), ProtocolError> {
        self.telemetry_channel_count = channel_ids.len();
        let payload = TelemetrySubscription {
            subscription_version: 7,
            sample_rate_hz,
            channel_ids,
        }
        .encode()?;
        let response = self.only_response(MessageType::TelemetrySubscribe, session_id, payload)?;
        if response.header.message_type == MessageType::TelemetrySubscribeAck {
            Ok(())
        } else {
            Err(response_error(&response))
        }
    }

    fn telemetry(&mut self) -> Result<TelemetryBatch, ProtocolError> {
        self.queue_device_tick()?;
        let response = self
            .drain_device_packets()?
            .into_iter()
            .find(|frame| frame.header.message_type == MessageType::TelemetryData)
            .ok_or(ProtocolError::Truncated)?;
        TelemetryBatch::decode(&response.payload, self.telemetry_channel_count)
    }

    fn inject_corrupt_next_device_packet(&mut self, offset: usize, mask: u8) {
        self.corrupt_next_device_packet = Some((offset, mask));
    }

    fn set_now_ms(&mut self, now_ms: u64) {
        self.now_ms = now_ms;
    }

    fn advance_ms(&mut self, elapsed_ms: u64) {
        self.now_ms = self.now_ms.saturating_add(elapsed_ms);
    }

    fn queue_device_tick(&mut self) -> Result<(), ProtocolError> {
        let responses = self.device.tick(self.now_ms);
        self.enqueue_device_responses(responses)
    }

    fn drain_device_packets(&mut self) -> Result<Vec<Frame>, ProtocolError> {
        let mut decoded_responses = Vec::new();
        while let Some(response) = self.tx_queue.pop() {
            let mut packet = encode_frame(&response)?;
            if let Some((offset, mask)) = self.corrupt_next_device_packet.take() {
                let byte = packet.get_mut(offset).ok_or(ProtocolError::InvalidLength)?;
                *byte ^= mask;
            }
            push_chunked(&mut self.host_decoder, &packet, &mut decoded_responses);
        }
        decoded_responses.into_iter().collect()
    }

    fn inject_log(&mut self, session_id: u32, sequence: u16) -> Result<(), ProtocolError> {
        let payload = LogMessage {
            timestamp_us: self.now_ms as u32 * 1_000,
            severity: LogSeverity::Info,
            module_id: 1,
            text: "queue pressure".into(),
        }
        .encode()?;
        let frame = Frame::new(
            MessageType::LogMessage,
            FrameFlags::RESPONSE,
            sequence,
            session_id,
            payload,
        )?;
        if self.tx_queue.push(Priority::Log, frame) == PushOutcome::Backpressure {
            return Err(ProtocolError::InvalidValue);
        }
        Ok(())
    }

    fn dropped_logs(&self) -> u64 {
        self.tx_queue.dropped_logs()
    }

    fn next_sequence(&mut self) -> u16 {
        let sequence = self.next_sequence;
        self.next_sequence = self.next_sequence.wrapping_add(1);
        sequence
    }

    fn only_response(
        &mut self,
        message_type: MessageType,
        session_id: u32,
        payload: Vec<u8>,
    ) -> Result<Frame, ProtocolError> {
        let sequence = self.next_sequence();
        self.only_response_with_sequence(message_type, session_id, payload, sequence)
    }

    fn only_response_with_sequence(
        &mut self,
        message_type: MessageType,
        session_id: u32,
        payload: Vec<u8>,
        sequence: u16,
    ) -> Result<Frame, ProtocolError> {
        let responses = self.exchange_with_sequence(message_type, session_id, payload, sequence)?;
        responses
            .into_iter()
            .find(|response| response.header.sequence == sequence)
            .ok_or(ProtocolError::Truncated)
    }

    fn exchange(
        &mut self,
        message_type: MessageType,
        session_id: u32,
        payload: Vec<u8>,
    ) -> Result<Vec<Frame>, ProtocolError> {
        let sequence = self.next_sequence();
        self.exchange_with_sequence(message_type, session_id, payload, sequence)
    }

    fn exchange_with_sequence(
        &mut self,
        message_type: MessageType,
        session_id: u32,
        payload: Vec<u8>,
        sequence: u16,
    ) -> Result<Vec<Frame>, ProtocolError> {
        self.now_ms = self.now_ms.saturating_add(1);
        let request = Frame::new(
            message_type,
            FrameFlags::ACK_REQUIRED,
            sequence,
            session_id,
            payload,
        )?;
        let packet = encode_frame(&request)?;

        let mut decoded_requests = Vec::new();
        push_chunked(&mut self.device_decoder, &packet, &mut decoded_requests);
        for decoded in decoded_requests {
            let responses = self.device.handle(decoded?, self.now_ms);
            self.enqueue_device_responses(responses)?;
        }

        let responses = self.drain_device_packets()?;
        if responses.is_empty() {
            return Err(ProtocolError::Truncated);
        }
        Ok(responses)
    }

    fn enqueue_device_responses(
        &mut self,
        responses: Vec<dctp_sim::QueuedFrame>,
    ) -> Result<(), ProtocolError> {
        for response in responses {
            match self.tx_queue.push(response.priority, response.frame) {
                PushOutcome::Backpressure => return Err(ProtocolError::InvalidValue),
                PushOutcome::DroppedTelemetry => self.device.note_telemetry_drop(1),
                PushOutcome::Enqueued | PushOutcome::DroppedLog => {}
            }
        }
        Ok(())
    }
}

fn push_chunked(
    decoder: &mut StreamDecoder,
    bytes: &[u8],
    output: &mut Vec<Result<Frame, ProtocolError>>,
) {
    const CHUNK_SIZES: [usize; 4] = [1, 2, 3, 5];
    let mut offset = 0;
    let mut chunk_index = 0;
    while offset < bytes.len() {
        let end = (offset + CHUNK_SIZES[chunk_index % CHUNK_SIZES.len()]).min(bytes.len());
        output.extend(decoder.push(&bytes[offset..end]));
        offset = end;
        chunk_index += 1;
    }
}

fn response_error(response: &Frame) -> ProtocolError {
    let Ok(error) = ErrorPayload::decode(&response.payload) else {
        return ProtocolError::InvalidValue;
    };
    match error.error_code {
        ErrorCode::InvalidSession => ProtocolError::InvalidSession,
        ErrorCode::RevisionConflict => ProtocolError::RevisionConflict,
        _ => ProtocolError::InvalidValue,
    }
}

#[test]
fn wire_session_survives_corruption_and_rejects_stale_write() {
    let mut harness = WireHarness::new();
    let session_a = harness.hello(0xAAAA).unwrap();
    harness.inject_corrupt_next_device_packet(3, 0x20);
    assert!(harness.heartbeat(session_a).is_err());
    assert!(harness.heartbeat(session_a).is_ok());
    let session_b = harness.hello(0xBBBB).unwrap();
    assert_ne!(session_a, session_b);
    assert!(harness.write_f32(session_a, 1, 0, 2.0).is_err());
    assert!(harness.write_f32(session_b, 1, 0, 2.0).is_ok());
}

#[test]
fn wire_handshake_manifest_read_write_and_telemetry_subscription_succeed() {
    let mut harness = WireHarness::new();
    let session = harness.hello(0x1020_3040).unwrap();

    let manifest = harness.manifest(session).unwrap();
    assert!(manifest
        .parameters
        .iter()
        .any(|parameter| parameter.param_id == 1));
    assert!(manifest.telemetry.len() >= 16);
    let before = harness.read_parameter(session, 1).unwrap();
    assert_eq!(before.value, ParamValue::F32(1.0));
    let accepted = harness.write_f32(session, 1, before.revision, 2.5).unwrap();
    assert_eq!(accepted.new_revision, 1);
    harness
        .subscribe(session, vec![200, 201, 202, 203])
        .unwrap();
}

#[test]
fn duplicate_wire_write_returns_one_revision_increment() {
    let mut harness = WireHarness::new();
    let session = harness.hello(0x7777).unwrap();
    let sequence = 55;

    let first = harness
        .write_f32_with_sequence(session, 1, 0, 3.0, sequence)
        .unwrap();
    let replay = harness
        .write_f32_with_sequence(session, 1, 0, 3.0, sequence)
        .unwrap();

    assert_eq!(first, replay);
    assert_eq!(first.new_revision, 1);
    assert_eq!(harness.read_parameter(session, 1).unwrap().revision, 1);
}

fn error_payload(response: &Frame) -> ErrorPayload {
    assert_eq!(response.header.message_type, MessageType::Error);
    ErrorPayload::decode(&response.payload).unwrap()
}

#[test]
fn commit_updates_flash_once_and_returns_generation() {
    let mut harness = WireHarness::new();
    let session = harness.hello(1).unwrap();
    let param_id = harness
        .manifest(session)
        .unwrap()
        .parameters
        .iter()
        .find(|descriptor| descriptor.machine_name == "pid.kp")
        .unwrap()
        .param_id;
    let before = harness.read_parameter(session, param_id).unwrap();
    let write = harness
        .write_f32(session, param_id, before.revision, 2.5)
        .unwrap();
    let sequence = 0x4242;
    let first = harness
        .commit_with_sequence(session, vec![(param_id, write.new_revision)], sequence)
        .unwrap();
    let retry = harness
        .commit_with_sequence(session, vec![(param_id, write.new_revision)], sequence)
        .unwrap();

    assert_eq!(before.persisted_value, Some(ParamValue::F32(1.0)));
    assert_eq!(first, retry);
    assert_eq!(first.storage_generation, 1);
    let after = harness.read_parameter(session, param_id).unwrap();
    assert_eq!(after.value, ParamValue::F32(2.5));
    assert_eq!(after.persisted_value, Some(ParamValue::F32(2.5)));
}

#[test]
fn failed_commit_keeps_flash_and_generation_while_retaining_ram_value() {
    for (failure, expected_error) in [
        (CommitFailure::Storage, ErrorCode::StorageFailed),
        (CommitFailure::Verify, ErrorCode::VerifyFailed),
    ] {
        let mut harness = WireHarness::new();
        let session = harness.hello(2).unwrap();
        let before = harness.read_parameter(session, 1).unwrap();
        let write = harness.write_f32(session, 1, before.revision, 2.5).unwrap();
        harness.device.set_commit_failure(Some(failure));

        let response = harness
            .commit_response_with_sequence(session, vec![(1, write.new_revision)], 0x5100)
            .unwrap();

        assert_eq!(error_payload(&response).error_code, expected_error);
        let after = harness.read_parameter(session, 1).unwrap();
        assert_eq!(after.value, ParamValue::F32(2.5));
        assert_eq!(after.persisted_value, Some(ParamValue::F32(1.0)));
        assert_eq!(harness.device.storage_generation(), 0);
    }
}

#[test]
fn reconnect_retains_committed_flash_value_and_generation() {
    let mut harness = WireHarness::new();
    let first_session = harness.hello(3).unwrap();
    let before = harness.read_parameter(first_session, 1).unwrap();
    let write = harness
        .write_f32(first_session, 1, before.revision, 2.5)
        .unwrap();
    harness
        .commit_with_sequence(first_session, vec![(1, write.new_revision)], 0x5200)
        .unwrap();

    harness.device.disconnect();
    let second_session = harness.hello(4).unwrap();
    let after = harness.read_parameter(second_session, 1).unwrap();

    assert_eq!(after.value, ParamValue::F32(2.5));
    assert_eq!(after.persisted_value, Some(ParamValue::F32(2.5)));
    assert_eq!(harness.device.storage_generation(), 1);
}

#[test]
fn default_manifest_supports_eight_of_at_least_sixteen_dynamic_channels() {
    let mut harness = WireHarness::new();
    let session = harness.hello(5).unwrap();
    let manifest = harness.manifest(session).unwrap();
    assert!(manifest.telemetry.len() >= 16);
    let ids = manifest
        .telemetry
        .iter()
        .take(8)
        .map(|descriptor| descriptor.channel_id)
        .collect();
    harness.subscribe_at(session, 500, ids).unwrap();
    harness.advance_ms(2);
    let first = harness.telemetry().unwrap();
    harness.advance_ms(2);
    let second = harness.telemetry().unwrap();

    assert_ne!(first.samples[0].values, second.samples[0].values);
}

#[test]
fn session_expires_at_exactly_3000_ms_over_the_wire() {
    let mut harness = WireHarness::new();
    let session = harness.hello(0xABC0).unwrap();

    let first_heartbeat_at_ms = 3_000;
    harness.set_now_ms(first_heartbeat_at_ms - 1);
    assert!(
        harness.heartbeat(session).is_ok(),
        "the 2999 ms boundary must keep the session valid"
    );

    let exact_expiration_at_ms = first_heartbeat_at_ms + 3_000;
    harness.set_now_ms(exact_expiration_at_ms - 1);
    assert!(
        harness.heartbeat(session).is_err(),
        "the exactly 3000 ms boundary must expire the session"
    );
}

#[test]
fn full_log_queue_still_delivers_heartbeat_and_parameter_ack() {
    let mut harness = WireHarness::with_queue_capacities([8, 32, 16, 1]);
    let session = harness.hello(0xD00D).unwrap();
    harness.inject_log(session, 90).unwrap();
    harness.inject_log(session, 91).unwrap();
    assert_eq!(harness.dropped_logs(), 1);

    assert!(harness.heartbeat(session).is_ok());
    assert!(harness.write_f32(session, 1, 0, 2.0).is_ok());
}

#[test]
fn mixed_telemetry_reports_drops_and_sequence_gap_after_queue_pressure() {
    let mut harness = WireHarness::with_queue_capacities([8, 32, 1, 16]);
    let session = harness.hello(0x5151).unwrap();
    harness
        .subscribe(session, vec![200, 201, 202, 203])
        .unwrap();

    for _ in 0..3 {
        harness.advance_ms(10);
        harness.queue_device_tick().unwrap();
    }
    let responses = harness.drain_device_packets().unwrap();
    assert_eq!(responses.len(), 1);
    assert_eq!(responses[0].header.message_type, MessageType::TelemetryData);
    let batch = TelemetryBatch::decode(&responses[0].payload, 4).unwrap();
    assert_eq!(batch.first_sample_sequence, 2);
    assert_eq!(batch.dropped_samples, 1);
    assert!(f32::from_bits(batch.samples[0].values[0]).is_finite());
    assert_eq!(batch.samples[0].values[1], 19);
    assert_eq!(batch.samples[0].values[2], 320);
    assert_eq!(batch.samples[0].values[3], 0);
}
