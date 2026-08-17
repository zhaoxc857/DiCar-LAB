/// Local UI/demo role used to exercise client-side gates.
///
/// This policy is not a distributed security boundary. A collaboration service and the
/// device must independently authorize operations in production.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AccessRole {
    Owner,
    Tuner,
    Observer,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LeaseState {
    Inactive,
    Active,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PermissionDecision {
    Allowed,
    Denied(&'static str),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AccessProfile {
    pub role: AccessRole,
    pub lease: LeaseState,
}

impl AccessProfile {
    pub const fn new(role: AccessRole, lease: LeaseState) -> Self {
        Self { role, lease }
    }

    pub const fn can_write(self) -> PermissionDecision {
        if matches!(self.role, AccessRole::Observer) {
            return PermissionDecision::Denied("仅观察者不能修改参数");
        }
        if matches!(self.lease, LeaseState::Inactive) {
            return PermissionDecision::Denied("当前设备没有活动控制租约");
        }
        PermissionDecision::Allowed
    }

    pub const fn can_commit(self) -> PermissionDecision {
        if !matches!(self.role, AccessRole::Owner) {
            return PermissionDecision::Denied("当前身份没有固化权限");
        }
        if matches!(self.lease, LeaseState::Inactive) {
            return PermissionDecision::Denied("当前设备没有活动控制租约");
        }
        PermissionDecision::Allowed
    }

    pub const fn can_flash(self) -> PermissionDecision {
        if !matches!(self.role, AccessRole::Owner) {
            return PermissionDecision::Denied("当前身份没有固件烧录权限");
        }
        if matches!(self.lease, LeaseState::Inactive) {
            return PermissionDecision::Denied("当前设备没有活动控制租约");
        }
        PermissionDecision::Allowed
    }
}
