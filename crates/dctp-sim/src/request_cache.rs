use std::collections::VecDeque;

use dctp_protocol::{Frame, MessageType};

pub const REQUEST_CACHE_CAPACITY: usize = 32;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RequestKey {
    pub session_id: u32,
    pub message_type: MessageType,
    pub sequence: u16,
}

#[derive(Debug, Default)]
pub struct RequestCache {
    entries: VecDeque<(RequestKey, Frame)>,
}

impl RequestCache {
    pub fn get_or_insert(&mut self, key: RequestKey, build: impl FnOnce() -> Frame) -> Frame {
        if let Some(frame) = self.get(&key) {
            return frame;
        }

        let frame = build();
        if self.entries.len() == REQUEST_CACHE_CAPACITY {
            self.entries.pop_front();
        }
        self.entries.push_back((key, frame.clone()));
        frame
    }

    pub(crate) fn get(&self, key: &RequestKey) -> Option<Frame> {
        self.entries
            .iter()
            .find(|(cached_key, _)| cached_key == key)
            .map(|(_, frame)| frame.clone())
    }

    pub(crate) fn insert(&mut self, key: RequestKey, frame: Frame) {
        let _ = self.get_or_insert(key, || frame);
    }

    pub(crate) fn clear(&mut self) {
        self.entries.clear();
    }
}
