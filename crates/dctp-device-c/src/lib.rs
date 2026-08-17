//! C 车端参考库（firmware/dctp-device）的交叉验证工具。
//!
//! build.rs 把 C 源编译进本 crate；这里提供安全封装，测试用 Rust 的
//! dctp-protocol 权威实现构造请求并校验 C 设备的响应字节。

use std::ffi::CString;
use std::os::raw::{c_char, c_int, c_void};

const TX_CAPTURE_CAPACITY: usize = 16384;
const BLOB_CAPTURE_CAPACITY: usize = 1024;
const GOLDEN_CAPACITY: usize = 2048;

extern "C" {
    fn dctp_shim_size() -> usize;
    fn dctp_shim_init(
        memory: *mut c_void,
        with_persist: c_int,
        with_tx_budget: c_int,
        with_flash: c_int,
    ) -> *mut c_void;
    fn dctp_shim_rx(shim: *mut c_void, bytes: *const u8, len: usize, now_ms: u32);
    fn dctp_shim_poll(shim: *mut c_void, now_ms: u32, now_us: u32);
    fn dctp_shim_take_tx(shim: *mut c_void, out: *mut u8, capacity: usize) -> usize;
    fn dctp_shim_set_tx_free(shim: *mut c_void, bytes: usize);
    fn dctp_shim_set_persist_result(shim: *mut c_void, result: c_int);
    fn dctp_shim_persist_calls(shim: *const c_void) -> u32;
    fn dctp_shim_last_blob(shim: *const c_void, out: *mut u8, capacity: usize) -> usize;
    fn dctp_shim_manifest_crc32(shim: *const c_void) -> u32;
    fn dctp_shim_storage_generation(shim: *const c_void) -> u32;
    fn dctp_shim_session_active(shim: *const c_void) -> c_int;
    fn dctp_shim_prepare_flash_calls(shim: *const c_void) -> u32;
    fn dctp_shim_take_flash_transition(
        shim: *mut c_void,
        operation_id: *mut u8,
        protocol: *mut u8,
        entry_delay_ms: *mut u16,
        initial_baud: *mut u32,
    ) -> c_int;
    fn dctp_shim_log(
        shim: *mut c_void,
        severity: u8,
        module_id: u16,
        timestamp_us: u32,
        text: *const c_char,
    ) -> c_int;
    fn dctp_shim_set_value_f32(shim: *mut c_void, param_id: u32, value: f32) -> c_int;
    fn dctp_shim_get_value_bits(
        shim: *const c_void,
        param_id: u32,
        type_out: *mut u8,
        bits_out: *mut u32,
    ) -> c_int;
    fn dctp_shim_storage_apply(
        shim: *mut c_void,
        slot_a: *const u8,
        len_a: u32,
        slot_b: *const u8,
        len_b: u32,
    ) -> c_int;
    fn dctp_shim_build_golden(which: c_int, out: *mut u8, capacity: usize) -> usize;
}

/// 由 shim 静态表驱动的一台 C 设备实例，捕获其全部串口输出。
pub struct TestDevice {
    _memory: Vec<u64>,
    shim: *mut c_void,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FlashTransition {
    pub operation_id: [u8; 16],
    pub bootloader_protocol: u8,
    pub entry_delay_ms: u16,
    pub initial_baud: u32,
}

impl TestDevice {
    pub fn new(with_persist: bool, with_tx_budget: bool) -> Self {
        let size = unsafe { dctp_shim_size() };
        let words = size.div_ceil(8);
        let mut memory = vec![0u64; words];
        let shim = unsafe {
            dctp_shim_init(
                memory.as_mut_ptr().cast(),
                c_int::from(with_persist),
                c_int::from(with_tx_budget),
                0,
            )
        };
        assert!(
            !shim.is_null(),
            "C device rejected the fixed descriptor tables"
        );
        Self {
            _memory: memory,
            shim,
        }
    }

    pub fn new_with_flash() -> Self {
        let size = unsafe { dctp_shim_size() };
        let words = size.div_ceil(8);
        let mut memory = vec![0u64; words];
        let shim = unsafe { dctp_shim_init(memory.as_mut_ptr().cast(), 0, 0, 1) };
        assert!(
            !shim.is_null(),
            "C device rejected the flash-enabled config"
        );
        Self {
            _memory: memory,
            shim,
        }
    }

    pub fn rx(&mut self, bytes: &[u8], now_ms: u32) {
        unsafe { dctp_shim_rx(self.shim, bytes.as_ptr(), bytes.len(), now_ms) };
    }

    pub fn poll(&mut self, now_ms: u32, now_us: u32) {
        unsafe { dctp_shim_poll(self.shim, now_ms, now_us) };
    }

    pub fn take_tx(&mut self) -> Vec<u8> {
        let mut buffer = vec![0u8; TX_CAPTURE_CAPACITY];
        let len = unsafe { dctp_shim_take_tx(self.shim, buffer.as_mut_ptr(), buffer.len()) };
        buffer.truncate(len);
        buffer
    }

    pub fn set_tx_free(&mut self, bytes: usize) {
        unsafe { dctp_shim_set_tx_free(self.shim, bytes) };
    }

    pub fn set_persist_result(&mut self, result: i32) {
        unsafe { dctp_shim_set_persist_result(self.shim, result) };
    }

    pub fn persist_calls(&self) -> u32 {
        unsafe { dctp_shim_persist_calls(self.shim) }
    }

    pub fn last_blob(&self) -> Vec<u8> {
        let mut buffer = vec![0u8; BLOB_CAPTURE_CAPACITY];
        let len = unsafe { dctp_shim_last_blob(self.shim, buffer.as_mut_ptr(), buffer.len()) };
        buffer.truncate(len);
        buffer
    }

    pub fn manifest_crc32(&self) -> u32 {
        unsafe { dctp_shim_manifest_crc32(self.shim) }
    }

    pub fn storage_generation(&self) -> u32 {
        unsafe { dctp_shim_storage_generation(self.shim) }
    }

    pub fn session_active(&self) -> bool {
        unsafe { dctp_shim_session_active(self.shim) != 0 }
    }

    pub fn prepare_flash_calls(&self) -> u32 {
        unsafe { dctp_shim_prepare_flash_calls(self.shim) }
    }

    pub fn take_flash_transition(&mut self) -> Option<FlashTransition> {
        let mut operation_id = [0u8; 16];
        let mut bootloader_protocol = 0u8;
        let mut entry_delay_ms = 0u16;
        let mut initial_baud = 0u32;
        let available = unsafe {
            dctp_shim_take_flash_transition(
                self.shim,
                operation_id.as_mut_ptr(),
                &mut bootloader_protocol,
                &mut entry_delay_ms,
                &mut initial_baud,
            )
        };
        (available != 0).then_some(FlashTransition {
            operation_id,
            bootloader_protocol,
            entry_delay_ms,
            initial_baud,
        })
    }

    pub fn log(&mut self, severity: u8, module_id: u16, timestamp_us: u32, text: &str) -> bool {
        let text = CString::new(text).expect("log text contains no NUL");
        unsafe { dctp_shim_log(self.shim, severity, module_id, timestamp_us, text.as_ptr()) != 0 }
    }

    pub fn set_value_f32(&mut self, param_id: u32, value: f32) -> bool {
        unsafe { dctp_shim_set_value_f32(self.shim, param_id, value) != 0 }
    }

    pub fn get_value_bits(&self, param_id: u32) -> Option<(u8, u32)> {
        let mut value_type = 0u8;
        let mut bits = 0u32;
        let found =
            unsafe { dctp_shim_get_value_bits(self.shim, param_id, &mut value_type, &mut bits) };
        (found != 0).then_some((value_type, bits))
    }

    pub fn storage_apply(&mut self, slot_a: Option<&[u8]>, slot_b: Option<&[u8]>) -> bool {
        let (ptr_a, len_a) = slot_a.map_or((std::ptr::null(), 0), |slot| {
            (slot.as_ptr(), slot.len() as u32)
        });
        let (ptr_b, len_b) = slot_b.map_or((std::ptr::null(), 0), |slot| {
            (slot.as_ptr(), slot.len() as u32)
        });
        unsafe { dctp_shim_storage_apply(self.shim, ptr_a, len_a, ptr_b, len_b) != 0 }
    }
}

/// 用与 generate_vectors.rs 相同的固定输入构造第 `which` 个黄金帧。
pub fn build_golden(which: i32) -> Vec<u8> {
    let mut buffer = vec![0u8; GOLDEN_CAPACITY];
    let len = unsafe { dctp_shim_build_golden(which, buffer.as_mut_ptr(), buffer.len()) };
    buffer.truncate(len);
    buffer
}
