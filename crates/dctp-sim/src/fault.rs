use dctp_protocol::ProtocolError;

pub const MAX_FAULT_RULES: usize = 64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Direction {
    HostToDevice,
    DeviceToHost,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FaultAction {
    Pass,
    Drop,
    Duplicate,
    CorruptByte { offset: usize, mask: u8 },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FaultRule {
    pub direction: Direction,
    pub packet_index: u64,
    pub action: FaultAction,
}

#[derive(Debug, Default)]
pub struct FaultInjector {
    rules: Vec<FaultRule>,
    host_to_device_index: u64,
    device_to_host_index: u64,
}

impl FaultInjector {
    pub fn new(rules: Vec<FaultRule>) -> Result<Self, ProtocolError> {
        if rules.len() > MAX_FAULT_RULES
            || rules.iter().enumerate().any(|(index, rule)| {
                rules[..index].iter().any(|previous| {
                    previous.direction == rule.direction
                        && previous.packet_index == rule.packet_index
                })
            })
        {
            return Err(ProtocolError::InvalidValue);
        }
        Ok(Self {
            rules,
            host_to_device_index: 0,
            device_to_host_index: 0,
        })
    }

    pub fn apply(&mut self, direction: Direction, packet: &[u8]) -> Vec<Vec<u8>> {
        let packet_index = match direction {
            Direction::HostToDevice => {
                let index = self.host_to_device_index;
                self.host_to_device_index = self.host_to_device_index.wrapping_add(1);
                index
            }
            Direction::DeviceToHost => {
                let index = self.device_to_host_index;
                self.device_to_host_index = self.device_to_host_index.wrapping_add(1);
                index
            }
        };
        let action = self
            .rules
            .iter()
            .find(|rule| rule.direction == direction && rule.packet_index == packet_index)
            .map(|rule| &rule.action)
            .unwrap_or(&FaultAction::Pass);

        match action {
            FaultAction::Pass => vec![packet.to_vec()],
            FaultAction::Drop => Vec::new(),
            FaultAction::Duplicate => vec![packet.to_vec(), packet.to_vec()],
            FaultAction::CorruptByte { offset, mask } => {
                let mut corrupted = packet.to_vec();
                if let Some(byte) = corrupted.get_mut(*offset) {
                    *byte ^= *mask;
                }
                vec![corrupted]
            }
        }
    }
}
