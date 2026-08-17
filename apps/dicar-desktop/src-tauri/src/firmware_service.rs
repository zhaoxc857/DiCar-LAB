use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use dicar_app_core::{Endpoint, OperationStatus, SerialHardwareProfile, SnapshotPhase};
use dicar_firmware_flash::bsl::{mspm0_crc32, Mspm0RomBsl};
use dicar_firmware_flash::credentials::CredentialStore;
use dicar_firmware_flash::package::{FirmwarePackage, TrustStore, VerifiedFirmwarePackage};
use dicar_firmware_flash::provisioning_store::ProvisioningStore;
use dicar_firmware_flash::recovery_store::RecoveryStore;
use dicar_firmware_flash::target::{FirmwareTargetAdapter, Mspm0g3507TmxAdapter};
use serde::{Deserialize, Serialize};
#[cfg(any(target_env = "msvc", feature = "native-check"))]
use tauri::Manager;
use uuid::Uuid;

use crate::{AppState, FirmwareUpgradeGuard};

pub trait FirmwareSerial: Read + Write + Send {}
impl<T: Read + Write + Send> FirmwareSerial for T {}

pub trait FirmwareTransportFactory: Send + Sync {
    fn wait_for_transition(&self);
    fn open(
        &self,
        port_name: &str,
        baud_rate: u32,
    ) -> Result<Box<dyn FirmwareSerial>, FirmwareFlashErrorDto>;
}

#[derive(Clone, Copy, Debug, Default)]
struct SystemFirmwareTransportFactory;

impl FirmwareTransportFactory for SystemFirmwareTransportFactory {
    fn wait_for_transition(&self) {
        std::thread::sleep(Duration::from_millis(250));
    }

    fn open(
        &self,
        port_name: &str,
        baud_rate: u32,
    ) -> Result<Box<dyn FirmwareSerial>, FirmwareFlashErrorDto> {
        let port = serialport::new(port_name, baud_rate)
            .data_bits(serialport::DataBits::Eight)
            .parity(serialport::Parity::None)
            .stop_bits(serialport::StopBits::One)
            .flow_control(serialport::FlowControl::None)
            .timeout(Duration::from_secs(2))
            .open()
            .map_err(|_| {
                FirmwareFlashErrorDto::new(
                    "bslSerialOpenFailed",
                    "无法以 9600 8N1 打开 ROM BSL 串口",
                )
            })?;
        Ok(Box::new(port))
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FirmwareFlashErrorDto {
    pub code: String,
    pub message: String,
    pub operation_id: Option<Uuid>,
}

impl FirmwareFlashErrorDto {
    fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            operation_id: None,
        }
    }

    fn with_operation(mut self, operation_id: Uuid) -> Self {
        self.operation_id = Some(operation_id);
        self
    }
}

impl std::fmt::Display for FirmwareFlashErrorDto {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for FirmwareFlashErrorDto {}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum FirmwareFlashPhase {
    Preparing,
    SwitchingTransport,
    Unlocking,
    Erasing,
    Programming,
    Verifying,
    Restarting,
    Reconnecting,
    Succeeded,
    RecoveryRequired,
    Retrying,
    RollingBack,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FirmwareFlashEvent {
    pub operation_id: Uuid,
    pub phase: FirmwareFlashPhase,
    pub progress_percent: u8,
    pub message: String,
}

fn run_bsl_update<T: Read + Write>(
    operation_id: Uuid,
    bsl: &mut Mspm0RomBsl<T>,
    image: &[u8],
    password: &[u8; 32],
    mut emit: impl FnMut(FirmwareFlashEvent),
) -> Result<(), FirmwareFlashErrorDto> {
    let adapter = Mspm0g3507TmxAdapter;
    let (erase_start, erase_end) = adapter
        .erase_range(0, image.len() as u32)
        .map_err(|_| flash_error(operation_id))?;

    emit(event(
        operation_id,
        FirmwareFlashPhase::Unlocking,
        35,
        "正在连接并解锁 TI ROM BSL",
    ));
    bsl.connect().map_err(|_| flash_error(operation_id))?;
    bsl.device_info().map_err(|_| flash_error(operation_id))?;
    bsl.unlock(password)
        .map_err(|_| flash_error(operation_id))?;

    emit(event(
        operation_id,
        FirmwareFlashPhase::Erasing,
        45,
        "正在擦除目标主 Flash 范围",
    ));
    bsl.erase_range(erase_start, erase_end)
        .map_err(|_| flash_error(operation_id))?;

    emit(event(
        operation_id,
        FirmwareFlashPhase::Programming,
        60,
        "正在分块写入固件镜像",
    ));
    bsl.program(0, image)
        .map_err(|_| flash_error(operation_id))?;

    emit(event(
        operation_id,
        FirmwareFlashPhase::Verifying,
        85,
        "正在校验设备端固件 CRC",
    ));
    bsl.verify_crc(0, image.len() as u32, mspm0_crc32(image))
        .map_err(|_| flash_error(operation_id))?;

    emit(event(
        operation_id,
        FirmwareFlashPhase::Restarting,
        92,
        "正在启动新固件",
    ));
    bsl.start_application()
        .map_err(|_| flash_error(operation_id))
}

fn event(
    operation_id: Uuid,
    phase: FirmwareFlashPhase,
    progress_percent: u8,
    message: impl Into<String>,
) -> FirmwareFlashEvent {
    FirmwareFlashEvent {
        operation_id,
        phase,
        progress_percent,
        message: message.into(),
    }
}

fn flash_error(operation_id: Uuid) -> FirmwareFlashErrorDto {
    FirmwareFlashErrorDto::new("bslOperationFailed", "TI ROM BSL 固件操作失败")
        .with_operation(operation_id)
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FirmwareFlashStartRequest {
    pub operation_id: Uuid,
    pub package_bytes: Vec<u8>,
    #[serde(default)]
    pub allow_downgrade: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FirmwarePackageSummary {
    pub release_id: Uuid,
    pub target: &'static str,
    pub mcu: &'static str,
    pub firmware_version: [u16; 3],
    pub image_length: u32,
    pub image_sha256: String,
    pub package_sha256: String,
    pub signing_key_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FirmwareFlashResult {
    pub operation_id: Uuid,
    pub device_id_hex: String,
    pub firmware_version: [u16; 3],
    pub rolled_back: bool,
}

struct RecoveryOperation {
    lease: FirmwareUpgradeGuard,
    device_id: [u8; 16],
    endpoint: Endpoint,
    candidate_package: Vec<u8>,
}

#[derive(Clone, Copy, Debug)]
struct ActiveOperation {
    operation_id: Uuid,
    cancel_requested: bool,
    prepare_started: bool,
}

pub struct FirmwareFlashServiceState {
    root: PathBuf,
    credentials: Arc<dyn CredentialStore>,
    transport_factory: Arc<dyn FirmwareTransportFactory>,
    active_operation: Mutex<Option<ActiveOperation>>,
    recovery_operations: Mutex<BTreeMap<Uuid, RecoveryOperation>>,
}

impl FirmwareFlashServiceState {
    pub fn new<C, F>(root: impl AsRef<Path>, credentials: Arc<C>, transport_factory: Arc<F>) -> Self
    where
        C: CredentialStore + 'static,
        F: FirmwareTransportFactory + 'static,
    {
        Self {
            root: root.as_ref().to_owned(),
            credentials,
            transport_factory,
            active_operation: Mutex::new(None),
            recovery_operations: Mutex::new(BTreeMap::new()),
        }
    }

    #[cfg(windows)]
    pub fn system(root: impl AsRef<Path>) -> Self {
        Self::new(
            root,
            Arc::new(dicar_firmware_flash::credentials::WindowsCredentialStore),
            Arc::new(SystemFirmwareTransportFactory),
        )
    }

    pub fn inspect(
        &self,
        device_id: &[u8; 16],
        package_bytes: &[u8],
    ) -> Result<FirmwarePackageSummary, FirmwareFlashErrorDto> {
        let (_trust, package) = self.verified_package(device_id, package_bytes)?;
        let manifest = package.manifest();
        Ok(FirmwarePackageSummary {
            release_id: manifest.release_id(),
            target: "lckfb-tmx-mspm0g3507",
            mcu: "MSPM0G3507",
            firmware_version: manifest.firmware_version(),
            image_length: manifest.image_length(),
            image_sha256: encode_hex(&manifest.image_sha256()),
            package_sha256: encode_hex(&package.package_sha256()),
            signing_key_id: manifest.signing_key_id().to_owned(),
        })
    }

    pub fn start(
        &self,
        app: &AppState,
        request: FirmwareFlashStartRequest,
        mut emit: impl FnMut(FirmwareFlashEvent) -> Result<(), FirmwareFlashErrorDto>,
    ) -> Result<FirmwareFlashResult, FirmwareFlashErrorDto> {
        let snapshot = app.snapshot();
        let Some(identity) = snapshot.transport_identity else {
            return Err(FirmwareFlashErrorDto::new(
                "deviceNotReady",
                "设备尚未连接并就绪",
            ));
        };
        if !matches!(identity.endpoint, Endpoint::Serial { .. }) {
            return Err(FirmwareFlashErrorDto::new(
                "realSerialRequired",
                "固件升级仅支持真实串口设备",
            ));
        }

        let operation_id = request.operation_id;
        if operation_id.is_nil() {
            return Err(firmware_error(
                "invalidFirmwareOperation",
                "固件升级操作 ID 无效",
                operation_id,
            ));
        }
        let lease = app.begin_firmware_upgrade().map_err(|_| {
            firmware_error(
                "firmwareUpgradeActive",
                "已有固件升级正在进行",
                operation_id,
            )
        })?;
        {
            let mut active = lock(&self.active_operation);
            if active.is_some() {
                return Err(firmware_error(
                    "firmwareUpgradeActive",
                    "已有固件升级正在进行",
                    operation_id,
                ));
            }
            *active = Some(ActiveOperation {
                operation_id,
                cancel_requested: false,
                prepare_started: false,
            });
        }
        let mut prepared = false;
        let mut recovery_context = None;
        let outcome = (|| {
            let snapshot = app.snapshot();
            if snapshot.phase != SnapshotPhase::Ready {
                return Err(firmware_error(
                    "deviceNotReady",
                    "设备尚未连接并就绪",
                    operation_id,
                ));
            }
            let endpoint = snapshot
                .transport_identity
                .as_ref()
                .map(|identity| identity.endpoint.clone())
                .ok_or_else(|| {
                    firmware_error("deviceNotReady", "设备尚未连接并就绪", operation_id)
                })?;
            let (port_name, baud_rate, hardware_profile) = match &endpoint {
                Endpoint::Serial {
                    port_name,
                    baud_rate,
                    hardware_profile,
                } => (port_name.clone(), *baud_rate, *hardware_profile),
                Endpoint::Simulator { .. } => {
                    return Err(firmware_error(
                        "realSerialRequired",
                        "固件升级仅支持真实串口设备",
                        operation_id,
                    ));
                }
            };
            if baud_rate != 9_600
                || !matches!(
                    hardware_profile,
                    SerialHardwareProfile::Hc05BluetoothSpp | SerialHardwareProfile::NanoUartWl
                )
            {
                return Err(firmware_error(
                    "unsupportedFirmwareLink",
                    "首版固件升级要求 HC-05 或 nanoUART-wl 使用 9600 8N1",
                    operation_id,
                ));
            }
            let device_id_hex = snapshot.device_id_hex.as_deref().ok_or_else(|| {
                firmware_error("deviceNotReady", "设备身份尚未加载", operation_id)
            })?;
            let device_id = parse_device_id(device_id_hex).ok_or_else(|| {
                firmware_error("invalidDeviceIdentity", "设备身份格式无效", operation_id)
            })?;
            let current_version = snapshot.firmware_version.ok_or_else(|| {
                firmware_error("deviceNotReady", "设备固件版本尚未加载", operation_id)
            })?;
            let (trust, package) = self
                .verified_package(&device_id, &request.package_bytes)
                .map_err(|error| error.with_operation(operation_id))?;
            let manifest = package.manifest();
            if manifest.firmware_version() < current_version && !request.allow_downgrade {
                return Err(firmware_error(
                    "downgradeConfirmationRequired",
                    "目标版本低于当前版本，需要显式确认降级",
                    operation_id,
                ));
            }
            let adapter = Mspm0g3507TmxAdapter;
            adapter
                .validate_image(0, manifest.image_length())
                .map_err(|_| {
                    firmware_error(
                        "targetImageRejected",
                        "固件镜像不符合目标 Flash 边界",
                        operation_id,
                    )
                })?;
            RecoveryStore::new(self.root.join("recovery"))
                .load(&device_id, &trust)
                .map_err(|_| {
                    firmware_error(
                        "recoveryPackageRequired",
                        "设备缺少已验证的恢复固件包",
                        operation_id,
                    )
                })?;
            let password = self.credentials.load(&device_id).map_err(|_| {
                firmware_error(
                    "deviceCredentialMissing",
                    "设备固件凭据不可用",
                    operation_id,
                )
            })?;

            recovery_context = Some((device_id, endpoint.clone(), request.package_bytes.clone()));
            emit_safely(
                &mut emit,
                event(
                    operation_id,
                    FirmwareFlashPhase::Preparing,
                    15,
                    "正在请求设备安全停机并切换到 ROM BSL",
                ),
            );
            {
                let mut active = lock(&self.active_operation);
                let state = active.as_mut().ok_or_else(|| {
                    firmware_error(
                        "firmwareOperationNotFound",
                        "固件升级操作不存在",
                        operation_id,
                    )
                })?;
                if state.cancel_requested {
                    return Err(firmware_error(
                        "firmwareOperationCancelled",
                        "固件升级已在安全切换前取消",
                        operation_id,
                    ));
                }
                state.prepare_started = true;
            }
            let prepare = app
                .dispatch_firmware(
                    &lease,
                    dicar_app_core::CoreCommand::PrepareFirmwareFlash {
                        flash_operation_id: *operation_id.as_bytes(),
                        target_id: adapter.target_id(),
                        firmware_version: manifest.firmware_version(),
                        image_len: manifest.image_length(),
                        image_sha256: manifest.image_sha256(),
                    },
                )
                .map_err(|_| {
                    firmware_error(
                        "preparationRejected",
                        "设备拒绝进入固件升级模式",
                        operation_id,
                    )
                })?;
            if prepare.status != OperationStatus::Succeeded {
                return Err(firmware_error(
                    "preparationRejected",
                    "设备拒绝进入固件升级模式",
                    operation_id,
                ));
            }
            prepared = true;

            emit_safely(
                &mut emit,
                event(
                    operation_id,
                    FirmwareFlashPhase::SwitchingTransport,
                    25,
                    "正在等待设备完成串口切换",
                ),
            );
            self.transport_factory.wait_for_transition();
            let serial = self.transport_factory.open(&port_name, 9_600)?;
            let mut bsl = adapter.create_bsl(serial);
            run_bsl_update(
                operation_id,
                &mut bsl,
                package.image(),
                password.expose_secret(),
                |event| emit_safely(&mut emit, event),
            )?;
            drop(bsl);

            emit_safely(
                &mut emit,
                event(
                    operation_id,
                    FirmwareFlashPhase::Reconnecting,
                    96,
                    "正在重新连接并核对设备身份",
                ),
            );
            self.transport_factory.wait_for_transition();
            let reconnect = app
                .dispatch_firmware(
                    &lease,
                    dicar_app_core::CoreCommand::ConnectTo {
                        endpoint: endpoint.clone(),
                    },
                )
                .map_err(|_| {
                    firmware_error(
                        "reconnectFailed",
                        "新固件启动后无法重新连接设备",
                        operation_id,
                    )
                })?;
            if reconnect.status != OperationStatus::Succeeded {
                return Err(firmware_error(
                    "reconnectFailed",
                    "新固件启动后无法重新连接设备",
                    operation_id,
                ));
            }
            let reconnected = app.snapshot();
            if reconnected.device_id_hex.as_deref() != Some(device_id_hex)
                || reconnected.firmware_version != Some(manifest.firmware_version())
            {
                return Err(firmware_error(
                    "reconnectIdentityMismatch",
                    "重连后的设备身份或固件版本不匹配",
                    operation_id,
                ));
            }
            RecoveryStore::new(self.root.join("recovery"))
                .save(&device_id, &request.package_bytes, &trust)
                .map_err(|_| {
                    firmware_error(
                        "recoveryStoreFailed",
                        "新固件已启动，但恢复副本保存失败",
                        operation_id,
                    )
                })?;
            emit_safely(
                &mut emit,
                event(
                    operation_id,
                    FirmwareFlashPhase::Succeeded,
                    100,
                    "固件升级完成并已重新连接",
                ),
            );
            Ok(FirmwareFlashResult {
                operation_id,
                device_id_hex: device_id_hex.to_owned(),
                firmware_version: manifest.firmware_version(),
                rolled_back: false,
            })
        })();

        self.clear_active_operation(operation_id);
        match outcome {
            Ok(result) => Ok(result),
            Err(error) if prepared => {
                let (device_id, endpoint, candidate_package) =
                    recovery_context.expect("prepared upgrade has recovery context");
                lock(&self.recovery_operations).insert(
                    operation_id,
                    RecoveryOperation {
                        lease,
                        device_id,
                        endpoint,
                        candidate_package,
                    },
                );
                emit_safely(
                    &mut emit,
                    event(
                        operation_id,
                        FirmwareFlashPhase::RecoveryRequired,
                        0,
                        "设备需要人工进入 BSL 后重试或回滚",
                    ),
                );
                Err(FirmwareFlashErrorDto::new(
                    "recoveryRequired",
                    "固件升级中断；设备升级锁保持，等待重试或回滚",
                )
                .with_operation(error.operation_id.unwrap_or(operation_id)))
            }
            Err(error) => Err(error),
        }
    }

    pub fn cancel(&self, operation_id: Uuid) -> Result<(), FirmwareFlashErrorDto> {
        let mut active = lock(&self.active_operation);
        if let Some(state) = active
            .as_mut()
            .filter(|state| state.operation_id == operation_id)
        {
            if state.prepare_started {
                return Err(firmware_error(
                    "firmwareCancellationUnavailable",
                    "设备安全切换已开始，不能再取消固件升级",
                    operation_id,
                ));
            }
            state.cancel_requested = true;
            return Ok(());
        }
        if lock(&self.recovery_operations).contains_key(&operation_id) {
            return Err(firmware_error(
                "firmwareCancellationUnavailable",
                "设备处于恢复模式，不能取消固件升级",
                operation_id,
            ));
        }
        Err(firmware_error(
            "firmwareOperationNotFound",
            "固件升级操作不存在",
            operation_id,
        ))
    }

    pub fn retry(
        &self,
        app: &AppState,
        operation_id: Uuid,
        emit: impl FnMut(FirmwareFlashEvent) -> Result<(), FirmwareFlashErrorDto>,
    ) -> Result<FirmwareFlashResult, FirmwareFlashErrorDto> {
        self.resume(app, operation_id, false, emit)
    }

    pub fn rollback(
        &self,
        app: &AppState,
        operation_id: Uuid,
        emit: impl FnMut(FirmwareFlashEvent) -> Result<(), FirmwareFlashErrorDto>,
    ) -> Result<FirmwareFlashResult, FirmwareFlashErrorDto> {
        self.resume(app, operation_id, true, emit)
    }

    fn resume(
        &self,
        app: &AppState,
        operation_id: Uuid,
        rollback: bool,
        mut emit: impl FnMut(FirmwareFlashEvent) -> Result<(), FirmwareFlashErrorDto>,
    ) -> Result<FirmwareFlashResult, FirmwareFlashErrorDto> {
        let Some(operation) = lock(&self.recovery_operations).remove(&operation_id) else {
            return Err(firmware_error(
                "firmwareOperationNotFound",
                "固件升级操作不存在",
                operation_id,
            ));
        };
        let outcome = (|| {
            let trust = ProvisioningStore::new(self.root.join("trust"))
                .load_trust_store(&operation.device_id)
                .map_err(|_| {
                    firmware_error(
                        "deviceNotProvisioned",
                        "设备尚未导入可信发布公钥",
                        operation_id,
                    )
                })?;
            let package = if rollback {
                RecoveryStore::new(self.root.join("recovery"))
                    .load(&operation.device_id, &trust)
                    .map_err(|_| {
                        firmware_error(
                            "recoveryPackageRequired",
                            "设备缺少已验证的恢复固件包",
                            operation_id,
                        )
                    })?
            } else {
                FirmwarePackage::inspect(&operation.candidate_package, &trust).map_err(|_| {
                    firmware_error(
                        "invalidFirmwarePackage",
                        "候选固件包无法重新验证",
                        operation_id,
                    )
                })?
            };
            let password = self.credentials.load(&operation.device_id).map_err(|_| {
                firmware_error(
                    "deviceCredentialMissing",
                    "设备固件凭据不可用",
                    operation_id,
                )
            })?;
            let port_name = match &operation.endpoint {
                Endpoint::Serial { port_name, .. } => port_name,
                Endpoint::Simulator { .. } => {
                    return Err(firmware_error(
                        "realSerialRequired",
                        "恢复操作仅支持真实串口设备",
                        operation_id,
                    ));
                }
            };
            emit_safely(
                &mut emit,
                event(
                    operation_id,
                    if rollback {
                        FirmwareFlashPhase::RollingBack
                    } else {
                        FirmwareFlashPhase::Retrying
                    },
                    10,
                    if rollback {
                        "正在刷回已验证的恢复固件"
                    } else {
                        "正在重试候选固件"
                    },
                ),
            );
            self.transport_factory.wait_for_transition();
            let serial = self.transport_factory.open(port_name, 9_600)?;
            let adapter = Mspm0g3507TmxAdapter;
            let mut bsl = adapter.create_bsl(serial);
            run_bsl_update(
                operation_id,
                &mut bsl,
                package.image(),
                password.expose_secret(),
                |event| emit_safely(&mut emit, event),
            )?;
            drop(bsl);

            emit_safely(
                &mut emit,
                event(
                    operation_id,
                    FirmwareFlashPhase::Reconnecting,
                    96,
                    "正在重新连接并核对设备身份",
                ),
            );
            self.transport_factory.wait_for_transition();
            let reconnect = app
                .dispatch_firmware(
                    &operation.lease,
                    dicar_app_core::CoreCommand::ConnectTo {
                        endpoint: operation.endpoint.clone(),
                    },
                )
                .map_err(|_| {
                    firmware_error(
                        "reconnectFailed",
                        "固件启动后无法重新连接设备",
                        operation_id,
                    )
                })?;
            if reconnect.status != OperationStatus::Succeeded {
                return Err(firmware_error(
                    "reconnectFailed",
                    "固件启动后无法重新连接设备",
                    operation_id,
                ));
            }
            let snapshot = app.snapshot();
            let expected_device_id = encode_hex(&operation.device_id);
            if snapshot.device_id_hex.as_deref() != Some(&expected_device_id)
                || snapshot.firmware_version != Some(package.manifest().firmware_version())
            {
                return Err(firmware_error(
                    "reconnectIdentityMismatch",
                    "重连后的设备身份或固件版本不匹配",
                    operation_id,
                ));
            }
            if !rollback {
                RecoveryStore::new(self.root.join("recovery"))
                    .save(&operation.device_id, &operation.candidate_package, &trust)
                    .map_err(|_| {
                        firmware_error(
                            "recoveryStoreFailed",
                            "固件已启动，但恢复副本保存失败",
                            operation_id,
                        )
                    })?;
            }
            emit_safely(
                &mut emit,
                event(
                    operation_id,
                    FirmwareFlashPhase::Succeeded,
                    100,
                    if rollback {
                        "恢复固件已刷回并重新连接"
                    } else {
                        "固件重试成功并已重新连接"
                    },
                ),
            );
            Ok(FirmwareFlashResult {
                operation_id,
                device_id_hex: expected_device_id,
                firmware_version: package.manifest().firmware_version(),
                rolled_back: rollback,
            })
        })();

        match outcome {
            Ok(result) => Ok(result),
            Err(error) => {
                lock(&self.recovery_operations).insert(operation_id, operation);
                emit_safely(
                    &mut emit,
                    event(
                        operation_id,
                        FirmwareFlashPhase::RecoveryRequired,
                        0,
                        "设备仍需人工进入 BSL 后重试或回滚",
                    ),
                );
                Err(FirmwareFlashErrorDto::new(
                    "recoveryRequired",
                    "恢复操作未完成；设备升级锁继续保持",
                )
                .with_operation(error.operation_id.unwrap_or(operation_id)))
            }
        }
    }

    fn clear_active_operation(&self, operation_id: Uuid) {
        let mut active = lock(&self.active_operation);
        if active
            .as_ref()
            .is_some_and(|state| state.operation_id == operation_id)
        {
            *active = None;
        }
    }

    fn verified_package(
        &self,
        device_id: &[u8; 16],
        package_bytes: &[u8],
    ) -> Result<(TrustStore, VerifiedFirmwarePackage), FirmwareFlashErrorDto> {
        let trust = ProvisioningStore::new(self.root.join("trust"))
            .load_trust_store(device_id)
            .map_err(|_| {
                FirmwareFlashErrorDto::new("deviceNotProvisioned", "设备尚未导入可信发布公钥")
            })?;
        let package = FirmwarePackage::inspect(package_bytes, &trust).map_err(|_| {
            FirmwareFlashErrorDto::new("invalidFirmwarePackage", "固件包验签或格式校验失败")
        })?;
        Ok((trust, package))
    }
}

#[cfg(any(target_env = "msvc", feature = "native-check"))]
#[tauri::command]
pub fn firmware_inspect(
    app: tauri::State<'_, AppState>,
    service: tauri::State<'_, FirmwareFlashServiceState>,
    package_bytes: Vec<u8>,
) -> Result<FirmwarePackageSummary, FirmwareFlashErrorDto> {
    let device_id = app
        .snapshot()
        .device_id_hex
        .as_deref()
        .and_then(parse_device_id)
        .ok_or_else(|| FirmwareFlashErrorDto::new("deviceNotReady", "设备身份尚未加载"))?;
    service.inspect(&device_id, &package_bytes)
}

#[cfg(any(target_env = "msvc", feature = "native-check"))]
#[tauri::command]
pub async fn firmware_start(
    app_handle: tauri::AppHandle,
    request: FirmwareFlashStartRequest,
    on_event: tauri::ipc::Channel<FirmwareFlashEvent>,
) -> Result<FirmwareFlashResult, FirmwareFlashErrorDto> {
    tauri::async_runtime::spawn_blocking(move || {
        let app = app_handle.state::<AppState>();
        let service = app_handle.state::<FirmwareFlashServiceState>();
        service.start(app.inner(), request, |event| {
            on_event.send(event).map_err(|_| {
                FirmwareFlashErrorDto::new("firmwareEventChannelClosed", "固件升级事件通道已关闭")
            })
        })
    })
    .await
    .map_err(|_| FirmwareFlashErrorDto::new("firmwareWorkerFailed", "固件升级任务异常退出"))?
}

#[cfg(any(target_env = "msvc", feature = "native-check"))]
#[tauri::command]
pub async fn firmware_retry(
    app_handle: tauri::AppHandle,
    operation_id: Uuid,
    on_event: tauri::ipc::Channel<FirmwareFlashEvent>,
) -> Result<FirmwareFlashResult, FirmwareFlashErrorDto> {
    tauri::async_runtime::spawn_blocking(move || {
        let app = app_handle.state::<AppState>();
        let service = app_handle.state::<FirmwareFlashServiceState>();
        service.retry(app.inner(), operation_id, |event| {
            on_event.send(event).map_err(|_| {
                FirmwareFlashErrorDto::new("firmwareEventChannelClosed", "固件升级事件通道已关闭")
            })
        })
    })
    .await
    .map_err(|_| FirmwareFlashErrorDto::new("firmwareWorkerFailed", "固件重试任务异常退出"))?
}

#[cfg(any(target_env = "msvc", feature = "native-check"))]
#[tauri::command]
pub async fn firmware_rollback(
    app_handle: tauri::AppHandle,
    operation_id: Uuid,
    on_event: tauri::ipc::Channel<FirmwareFlashEvent>,
) -> Result<FirmwareFlashResult, FirmwareFlashErrorDto> {
    tauri::async_runtime::spawn_blocking(move || {
        let app = app_handle.state::<AppState>();
        let service = app_handle.state::<FirmwareFlashServiceState>();
        service.rollback(app.inner(), operation_id, |event| {
            on_event.send(event).map_err(|_| {
                FirmwareFlashErrorDto::new("firmwareEventChannelClosed", "固件升级事件通道已关闭")
            })
        })
    })
    .await
    .map_err(|_| FirmwareFlashErrorDto::new("firmwareWorkerFailed", "固件回滚任务异常退出"))?
}

#[cfg(any(target_env = "msvc", feature = "native-check"))]
#[tauri::command]
pub fn firmware_cancel(
    service: tauri::State<'_, FirmwareFlashServiceState>,
    operation_id: Uuid,
) -> Result<(), FirmwareFlashErrorDto> {
    service.cancel(operation_id)
}

fn emit_safely(
    emit: &mut impl FnMut(FirmwareFlashEvent) -> Result<(), FirmwareFlashErrorDto>,
    event: FirmwareFlashEvent,
) {
    let _ = emit(event);
}

fn firmware_error(
    code: &'static str,
    message: &'static str,
    operation_id: Uuid,
) -> FirmwareFlashErrorDto {
    FirmwareFlashErrorDto::new(code, message).with_operation(operation_id)
}

fn parse_device_id(value: &str) -> Option<[u8; 16]> {
    if value.len() != 32 {
        return None;
    }
    let mut output = [0u8; 16];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        output[index] = hex_nibble(pair[0])?.checked_mul(16)? + hex_nibble(pair[1])?;
    }
    Some(output)
}

fn hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn encode_hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(DIGITS[usize::from(byte >> 4)] as char);
        output.push(DIGITS[usize::from(byte & 0x0F)] as char);
    }
    output
}

#[cfg(test)]
mod tests {
    use std::io::{Cursor, Read, Write};

    use dicar_firmware_flash::bsl::{mspm0_crc32, Mspm0RomBsl};

    use super::{run_bsl_update, FirmwareFlashEvent, FirmwareFlashPhase};

    #[derive(Default)]
    struct FakeSerial {
        reads: Cursor<Vec<u8>>,
        writes: Vec<u8>,
    }

    impl Read for FakeSerial {
        fn read(&mut self, output: &mut [u8]) -> std::io::Result<usize> {
            self.reads.read(output)
        }
    }

    impl Write for FakeSerial {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.writes.extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    fn response(payload: &[u8]) -> Vec<u8> {
        let mut packet = vec![0x08];
        packet.extend_from_slice(&(payload.len() as u16).to_le_bytes());
        packet.extend_from_slice(payload);
        packet.extend_from_slice(&mspm0_crc32(payload).to_le_bytes());
        packet
    }

    fn status() -> Vec<u8> {
        let mut bytes = vec![0x00];
        bytes.extend_from_slice(&response(&[0x3B, 0x00]));
        bytes
    }

    fn identity() -> Vec<u8> {
        let mut payload = vec![0x31];
        payload.extend_from_slice(&1u16.to_le_bytes());
        payload.extend_from_slice(&2u16.to_le_bytes());
        payload.extend_from_slice(&3u32.to_le_bytes());
        payload.extend_from_slice(&0x1331u16.to_le_bytes());
        payload.extend_from_slice(&133u16.to_le_bytes());
        payload.extend_from_slice(&0x2020_0000u32.to_le_bytes());
        payload.extend_from_slice(&0x1111_1111u32.to_le_bytes());
        payload.extend_from_slice(&0x2222_2222u32.to_le_bytes());
        let mut bytes = vec![0x00];
        bytes.extend_from_slice(&response(&payload));
        bytes
    }

    fn verification(crc: u32) -> Vec<u8> {
        let mut payload = vec![0x32];
        payload.extend_from_slice(&crc.to_le_bytes());
        let mut bytes = vec![0x00];
        bytes.extend_from_slice(&response(&payload));
        bytes
    }

    #[test]
    fn bsl_update_emits_critical_phases_in_order() {
        let image = vec![0xA5; 1024];
        let mut reads = vec![0x00];
        reads.extend_from_slice(&identity());
        reads.extend_from_slice(&status());
        reads.extend_from_slice(&status());
        for _ in 0..8 {
            reads.extend_from_slice(&status());
        }
        reads.extend_from_slice(&verification(mspm0_crc32(&image)));
        reads.push(0x00);
        let mut bsl = Mspm0RomBsl::new(FakeSerial {
            reads: Cursor::new(reads),
            writes: Vec::new(),
        });
        let mut events = Vec::<FirmwareFlashEvent>::new();
        let operation_id = uuid::Uuid::nil();

        run_bsl_update(operation_id, &mut bsl, &image, &[0x44; 32], |event| {
            events.push(event)
        })
        .unwrap();

        assert_eq!(
            events.iter().map(|event| event.phase).collect::<Vec<_>>(),
            vec![
                FirmwareFlashPhase::Unlocking,
                FirmwareFlashPhase::Erasing,
                FirmwareFlashPhase::Programming,
                FirmwareFlashPhase::Verifying,
                FirmwareFlashPhase::Restarting,
            ]
        );
    }
}
