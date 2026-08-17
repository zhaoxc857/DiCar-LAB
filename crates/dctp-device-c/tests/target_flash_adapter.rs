use std::ffi::c_void;

use dctp_device_c::FlashTransition;
use dctp_protocol::{
    encode_frame, FirmwareTargetId, Frame, FrameFlags, Hello, HelloAck, MessageType, PrepareFlash,
    PrepareFlashAck, StreamDecoder, WireDecode, WireEncode,
};

unsafe extern "C" {
    fn tmx_flash_shim_size() -> usize;
    fn tmx_flash_shim_init(memory: *mut c_void) -> *mut c_void;
    fn tmx_flash_shim_rx(shim: *mut c_void, bytes: *const u8, len: usize, now_ms: u32);
    fn tmx_flash_shim_take_tx(shim: *mut c_void, out: *mut u8, capacity: usize) -> usize;
    fn tmx_flash_shim_reset_order(shim: *mut c_void);
    fn tmx_flash_shim_set_tx_complete(shim: *mut c_void, complete: i32);
    fn tmx_flash_shim_poll_transition(shim: *mut c_void) -> i32;
    fn tmx_flash_shim_take_order(shim: *const c_void, out: *mut u8, capacity: usize) -> usize;
    fn tmx_flash_shim_enter_calls(shim: *const c_void) -> u32;
}

fn request(message_type: MessageType, sequence: u16, session_id: u32, payload: Vec<u8>) -> Vec<u8> {
    encode_frame(
        &Frame::new(
            message_type,
            FrameFlags::ACK_REQUIRED,
            sequence,
            session_id,
            payload,
        )
        .unwrap(),
    )
    .unwrap()
}

fn take_frame(shim: *mut c_void) -> Frame {
    let mut bytes = vec![0u8; 2048];
    let len = unsafe { tmx_flash_shim_take_tx(shim, bytes.as_mut_ptr(), bytes.len()) };
    bytes.truncate(len);
    let mut frames = StreamDecoder::new().push(&bytes);
    assert_eq!(frames.len(), 1);
    frames.remove(0).unwrap()
}

#[test]
fn tianmengxing_adapter_stops_safely_acks_then_waits_for_uart_before_one_shot_bsl_entry() {
    let _link_native_harness = std::mem::size_of::<FlashTransition>();
    let size = unsafe { tmx_flash_shim_size() };
    let mut memory = vec![0u64; size.div_ceil(8)];
    let shim = unsafe { tmx_flash_shim_init(memory.as_mut_ptr().cast()) };
    assert!(!shim.is_null());

    let hello = Hello {
        client_nonce: 7,
        min_version: 1,
        max_version: 1,
        max_payload: 1024,
    };
    let bytes = request(MessageType::Hello, 1, 0, hello.encode().unwrap());
    unsafe { tmx_flash_shim_rx(shim, bytes.as_ptr(), bytes.len(), 0) };
    let hello = HelloAck::decode(&take_frame(shim).payload).unwrap();
    unsafe { tmx_flash_shim_reset_order(shim) };

    let operation_id = [0xA5; 16];
    let prepare = PrepareFlash {
        operation_id,
        target_id: FirmwareTargetId::LCKFB_TMX_MSPM0G3507,
        firmware_version: [2, 0, 0],
        image_len: 4096,
        image_sha256: [0x33; 32],
    };
    let bytes = request(
        MessageType::PrepareFlash,
        2,
        hello.session_id,
        prepare.encode().unwrap(),
    );
    unsafe { tmx_flash_shim_rx(shim, bytes.as_ptr(), bytes.len(), 10) };
    let ack = PrepareFlashAck::decode(&take_frame(shim).payload).unwrap();
    assert_eq!(ack.operation_id, operation_id);
    assert_eq!(ack.entry_delay_ms, 250);
    assert_eq!(ack.initial_baud, 9_600);
    assert_eq!(take_order(shim), vec![1, 2]);

    assert_eq!(unsafe { tmx_flash_shim_poll_transition(shim) }, 0);
    assert_eq!(unsafe { tmx_flash_shim_enter_calls(shim) }, 0);
    unsafe { tmx_flash_shim_set_tx_complete(shim, 1) };
    assert_eq!(unsafe { tmx_flash_shim_poll_transition(shim) }, 1);
    assert_eq!(unsafe { tmx_flash_shim_enter_calls(shim) }, 1);
    assert_eq!(unsafe { tmx_flash_shim_poll_transition(shim) }, 0);
    assert_eq!(unsafe { tmx_flash_shim_enter_calls(shim) }, 1);
    assert_eq!(take_order(shim), vec![1, 2, 3, 3, 4]);
}

fn take_order(shim: *mut c_void) -> Vec<u8> {
    let mut order = vec![0u8; 16];
    let len = unsafe { tmx_flash_shim_take_order(shim, order.as_mut_ptr(), order.len()) };
    order.truncate(len);
    order
}
