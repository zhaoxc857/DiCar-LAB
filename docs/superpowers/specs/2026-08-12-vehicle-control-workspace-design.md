# DiCar Tune App 车辆与控制环工作区设计规格

- 状态：待用户复核
- 日期：2026-08-12
- 主产品：Windows 桌面 App
- 参考项目：`zbk666/IKUN-CAR-LAB` 的车型 YAML 插件与 Speed/Heading/Custom Loop 工作流

## 1. 目标

在现有 Windows App、DCTP Manifest 参数真值和高级波形工作台上，增加可扩展的车辆与控制环工作区。用户选择车型后，可以按速度环、航向环、底盘轴、电机、编码器等调车任务进入对应页面；页面自动组合相关参数、目标/反馈/误差/输出遥测和推荐波形。

本阶段采用混合架构：

- DCTP Manifest 始终是设备类型、范围、单位、可写性、危险标记、Revision、RAM/Flash 值和遥测通道的唯一设备真值。
- YAML 车辆配置只描述这些 Manifest 字段之间的语义关系和界面组织。
- 没有配置、配置不兼容或部分字段缺失时，App 继续提供 Manifest 自动生成的通用工作区，不能阻断连接、诊断、参数编辑或波形查看。

## 2. 参考项目做法与采用边界

IKUN-CAR-LAB 扫描 `vehicles/*.yaml` 和 `vehicles/*/config.yaml`，由 YAML 声明车型身份、连接默认值、速度/航向/自定义环字段、底盘轴、电机、参数列表和示波器预设；固定的 PySide6 Lab 页面按字符串 key 读取 JSON 遥测和发送 SET/CMD。

DiCar 采用其“车型配置负责语义映射、页面负责可复用交互”的产品思想，但不复制下列实现：

- 不使用任意字符串 JSON key 作为设备真值。
- 不允许 YAML 覆盖 Manifest 的参数类型、范围、权限、单位或危险性。
- 不在本阶段增加任意字符串 `CMD` 通道；目标写入只能引用 Manifest 中已存在的可写参数，并继续走 Revision-aware DCTP 参数写入。
- 不让车型配置决定真实串口是否安全；现有硬件档案、波特率探测和链路预算继续独立生效。

## 3. 方案选择

评估三种方案：

1. 仅靠名称规则从 Manifest 猜测控制环：零配置，但无法稳定表达级联环、四轮轴映射和车型特定波形。
2. **Manifest 真值 + YAML 语义覆盖 + 通用回退（采用）**：兼容现有设备，同时获得可贡献的车型插件能力。
3. 立即扩展 DCTP Manifest schema：设备自描述最完整，但会同时改变固件 SDK、模拟器、协议向量和 App，超出本阶段必要范围。

采用方案 2。本阶段不修改 DCTP、Rust `AppActor` 或 `DesktopBridge` 命令合同。

## 4. 车辆配置模型

### 4.1 文件来源

App 支持两类配置：

- 内置配置：随 App 源码和安装包发布，首个配置为 DiCar 两轮差速/速度控制示例。
- 用户导入配置：通过车型管理入口选择 `.yaml` 或 `.yml` 文件，解析成功后保存到 App 本地设置。导入只复制配置文本，不持续持有原文件路径。

始终存在一个不可删除的“通用 Manifest 工作区”。它不是 YAML 文件，不包含车型假设。

用户配置不能覆盖同 ID 的内置配置；再次导入同 ID 的用户配置时明确提示替换。最多保存 16 个用户配置，总文本不超过 2 MiB。单文件最大 256 KiB，schema v1 最多包含 32 个控制环、32 个参数分区和 32 个波形预设，每个列表最多引用 64 个字段。

### 4.2 YAML schema v1

```yaml
schema_version: 1

vehicle:
  id: dicar_diff_drive
  display_name: DiCar 两轮差速车
  type: 两轮差速 / 速度控制
  order: 10

control_loops:
  - id: speed
    label: 速度环
    category: 驱动控制
    hint: 目标速度、实际速度、误差与左右 PWM
    gains:
      Kp: pid.kp
    telemetry:
      target: drive.target_speed_mps
      feedback: drive.speed_mps
      error: drive.speed_error_mps
      outputs:
        - motor.left_pwm
        - motor.right_pwm
    recommended_channels:
      - drive.target_speed_mps
      - drive.speed_mps
      - drive.speed_error_mps
      - motor.left_pwm
      - motor.right_pwm

parameter_sections:
  - id: encoder
    label: 编码器与车轮
    parameters:
      - encoder.left.ppr
      - encoder.right.ppr
      - encoder.quadrature_multiplier
      - drive.wheel_diameter_mm
      - drive.gear_ratio

scope_presets:
  - id: drivetrain
    label: 驱动总览
    channels:
      - drive.speed_mps
      - drive.left_wheel_speed_mps
      - drive.right_wheel_speed_mps
```

所有参数引用按 `ParameterSnapshot.machineName` 精确匹配；所有遥测引用按 `TelemetryDescriptor.machineName` 精确匹配。参数和遥测处于不同命名空间。`id` 使用稳定的小写 ASCII、数字、短横线或下划线；显示文本允许 UTF-8。

### 4.3 控制环表达

每个 `control_loops` 项独立表达一个可调环。速度、航向外环、角速度内环、Vx/Vy/Wz 和四轮电机环都使用同一结构，不为车型写专属 React 页面。

- `target_parameter` 可选。存在且解析为可写数值参数时，页面显示目标设置控件并使用现有 RAM 写入流程；缺失时只显示遥测目标。
- `gains` 可包含 Kp、Ki、Kd 或其他命名增益。每项必须解析为 Manifest 参数；页面仍使用描述符自己的类型、范围、步长和权限。
- `telemetry.target`、`feedback`、`error` 可选，`outputs` 可为多通道。解析成功的角色会显示实时值和语义标签。
- `recommended_channels` 定义进入该环时的待应用波形选择。若省略，则按 target、feedback、error、outputs 的顺序生成并去重。
- 多级控制通过多个环和相同 `category` 表达，例如“航向外环”和“角速度内环”；首版不引入隐式级联运算。

### 4.4 参数分区与波形预设

`parameter_sections` 负责把编码器、车轮、电机保护、转向校准等 Manifest 参数组织成任务入口。未被任何配置引用的参数仍出现在“全部参数”中。

`scope_presets` 扩展现有波形工作组。配置预设优先显示在自动语义工作组之前，但仍受最多 8 路和当前链路预算限制；裁剪必须显示省略数量。选择预设只改变待应用通道，不自动发送订阅。

## 5. 解析、校验与兼容性

### 5.1 两级校验

第一级是文件结构校验，在导入时完成：

- 使用直接声明的 `yaml` 前端依赖和 core schema；禁止自定义标签、锚点、别名和 merge key，并受文件大小、配置数量和集合数量上限约束。
- `schema_version` 必须为 1。
- 车型 ID、控制环 ID、分区 ID 和预设 ID 必须合法且在各自集合中唯一。
- 引用必须是非空字符串，列表不得出现重复字段。
- 结构错误会拒绝导入，保留当前已选配置并显示精确路径和错误原因。

第二级是 Manifest 绑定校验，每次连接、重连、Manifest 更新或车型切换时重新执行：

- 参数/遥测 machine name 是否存在且命名空间正确。
- `target_parameter` 是否为可写数值参数；不可写时保留显示但禁用目标写入。
- gain 是否为数值参数；类型不符时该 gain 不可编辑。
- 推荐通道和预设通道是否存在。
- 控制环是否至少拥有一个有效参数或遥测角色。

绑定问题不拒绝车型文件。解析器产生按 `error`、`warning`、`info` 分级的兼容性报告；无任何有效内容的配置自动回退通用工作区，部分兼容配置只隐藏或禁用失配字段。

### 5.2 配置不能改变的真值

YAML 中不提供参数默认值、数值范围、步长、类型、单位、读写权限、持久化能力、危险标记或 Revision。显示名称也优先采用 Manifest；YAML 只为控制环、分区、角色和提示提供界面级标签。

用户配置永远不能：

- 使只读参数变为可写。
- 扩大设备范围或跳过危险确认。
- 把遥测字段当作参数写入。
- 绕过 Owner/Tuner/Observer 和控制租约。
- 直接向设备发送未在 Bridge 合同中的任意命令。

## 6. App 架构

新增独立的纯逻辑层：

```text
内置 YAML / 用户导入 YAML
  -> parseVehicleProfile
  -> validateVehicleProfileShape
  -> VehicleProfileStore

VehicleProfile + AppSnapshot Manifest DTO
  -> resolveVehicleWorkspace
  -> ResolvedVehicleWorkspace + compatibility issues
  -> LiveWorkbenchPage
```

建议文件边界：

- `vehicleProfiles/types.ts`：schema v1、解析后配置、解析完成工作区和兼容性问题类型。
- `vehicleProfiles/parser.ts`：受限 YAML 解析和结构校验。
- `vehicleProfiles/resolver.ts`：按 machine name 绑定当前参数/遥测描述符，不产生副作用。
- `vehicleProfiles/builtins/*.yaml`：随安装包发布的内置车型。
- `stores/vehicleProfileStore.ts`：选中 ID、用户导入文本和移除操作；只持久化配置，不复制 Manifest 状态。
- `components/vehicleProfiles/VehicleProfileManager.tsx`：导入、替换、删除和兼容性报告。
- `components/workbench/ControlLoopWorkspace.tsx`：控制环角色、增益和目标参数的组合视图。
- `components/workbench/WorkspaceNav.tsx`：控制环、参数分区和全部参数导航。

`resolveVehicleWorkspace` 是唯一把 YAML 引用转换为 `paramId`/`channelId` 的位置。其他组件只消费已解析 ID，避免在 UI 内重复字符串查找。

父页面与波形面板之间新增单向的 `WaveformSelectionRequest = { requestId; label; channelIds }`。只有用户选择另一个控制环或预设时才生成新 `requestId`；`WaveformPanel` 消费一次请求、按链路预算裁剪并更新待应用通道。后续父页面重渲染不能覆盖用户在波形面板内的手工选择。

## 7. 页面与交互

### 7.1 车型选择与管理

顶部现有仅含 `car-01` 占位数据的“车辆”选择器改为真实“车型配置”选择器：

- 第一项固定为“通用 Manifest”。
- 其后按 `vehicle.order`、显示名排序内置和用户车型。
- 旁边提供“管理车型”入口，用于导入、查看来源/兼容性和删除用户配置。
- 车型选择保存在本地，并在重启后恢复；所选配置不存在时回退通用 Manifest。

现有 `settingsStore.vehicleId` 是未实现多车功能的占位字段，不代表设备身份。迁移时将其替换为 `vehicleProfileId`；旧的 `car-01` 值映射到通用 Manifest。未来真实多车辆/多会话选择器必须使用独立状态，不复用车型配置 ID。

设备目前没有可用于可靠自动识别车型配置的 profile ID，因此 App 不根据模糊匹配静默切换。连接后只显示兼容性状态和建议，最终选择权属于用户。

### 7.2 工作区导航

工作台保持现有三列结构：

- 左列：任务导航。依次显示控制环、参数分区、未分类/全部参数。
- 中列：选中控制环的目标、PID/增益、实时角色值和兼容性说明；参数分区继续复用现有类型化参数控件和编码器校准组件。
- 右列：复用当前高级波形面板。

切换控制环时：

1. 更新中列所选任务。
2. 把该环推荐通道设置为波形“待应用”选择，并显示当前硬件预算裁剪结果。
3. 不自动调用 `setTelemetrySubscription`；用户仍需点击“应用 N Hz 订阅”。
4. 不自动写目标值、增益或 Flash。

### 7.3 控制环编辑

控制环页面显示：

- 环名称、类别、车型提示和绑定状态。
- target/feedback/error/output 的实时值、单位和缺失状态。
- `target_parameter` 的类型化输入与“写入 RAM”操作（若配置且允许）。
- 所有有效 gain 的类型化输入；继续采用设备 ACK 作为 RAM 真值。
- RAM/Flash/Revision、危险参数确认、撤销、固化审阅和 Observer/Tuner/Owner 权限全部复用现有合同。

本阶段不实现自动阶跃、自动 PID、批量立即发送或任意 CMD。用户的每次参数变更仍是显式操作。

### 7.4 通用 Manifest 回退

通用模式保留现有能力并改进组织：

- 参数按 Manifest `group` 展示。
- 波形继续使用自动语义工作组。
- 仅当同一参数组内存在可确定的 Kp/Ki/Kd machine-name 组合时，可显示“可能的 PID 组”建议；建议不获得比普通参数更高的写入权限。
- 配置失配时提供“一键切换到通用工作区”，当前设备连接和缓冲不被清除。

## 8. 状态、生命周期与错误处理

- 车型配置是本地项目元数据，不写入车辆 RAM 或 Flash。
- 连接前可以选择和管理配置；此时兼容性显示“等待 Manifest”。
- 连接成功或 Manifest 变化后重新解析工作区，保留仍存在的任务选择；任务消失时选择第一个有效任务。
- 切换车型不清除设备缓冲、A/B 游标或已生效订阅，只更新待应用推荐通道和页面组织。
- 导入失败、存储配额失败或用户配置损坏时，保留上一个有效内存状态并显示非破坏性错误。
- 删除当前用户配置前确认；删除成功后切换通用 Manifest。
- 配置兼容性错误不得被当作设备连接错误，也不得改变 AppSnapshot。

## 9. 测试与验收

### 9.1 纯逻辑测试

- 内置和用户 YAML 的 schema v1 解析。
- 文件大小、数量、非法 ID、重复 ID、重复引用和未知 schema 的拒绝路径。
- 对参数与遥测命名空间的精确绑定。
- 只读/非数值 target、非数值 gain、缺失遥测和部分兼容配置的分级报告。
- 控制环推荐通道默认推导、显式顺序、去重和 Manifest channel ID 解析。
- 配置无有效任务时回退通用工作区。
- 用户配置不能覆盖内置 ID，用户同 ID 替换必须显式确认。

### 9.2 React/Vitest 测试

- 车型选择器展示通用、内置和导入车型，并恢复持久化选择。
- 导入错误不会破坏当前车型；删除当前用户车型回退通用模式。
- 连接模拟器后速度环显示目标、反馈、误差、输出和有效增益。
- 选择控制环只更新待应用波形；点击应用后才调用 Bridge 订阅。
- target/gain 继续遵守类型、范围、Revision、权限和危险确认。
- 失配配置显示兼容性问题，仍可进入全部参数和通用波形。
- Manifest 更新后任务和通道安全重算，不留下陈旧 paramId/channelId。

### 9.3 App 回归

- 1280 x 720 和 200% 缩放下可选择车型、导航任务、编辑参数并查看波形。
- 纯键盘可以完成车型选择、任务导航、RAM 写入、波形 A/B 和订阅应用。
- 现有真实 COM、HC-05/nanoUART 硬件档案、链路预算、诊断、RAM/Flash、窗口关闭保护全部回归通过。
- YAML 内置资源进入 Vite 生产构建和 Tauri NSIS 包；安装后无需源码目录即可载入内置车型。
- 前端 lint、typecheck、全量 Vitest、production build 和 Playwright E2E 全部通过；若修改桌面资源装载，再执行 Tauri native check/NSIS 构建。

## 10. 实施顺序

实现严格遵循 RED -> GREEN -> REFACTOR：

1. schema 类型、受限 YAML 解析和结构错误。
2. Manifest resolver、兼容性报告与通用回退。
3. 内置车型、持久化 store 和车型管理入口。
4. 工作区导航与控制环组合视图。
5. 控制环到波形待应用工作组的接线。
6. 响应式、键盘、导入、Manifest 变化和桌面打包回归。

## 11. 本阶段边界

本阶段不实现：

- DCTP Manifest schema 变更或 MCU 下发车型配置。
- 任意字符串 `CMD`、自动阶跃测试或自动 PID。
- 参数方案、实验档案和曲线比较。
- 赛道/弯道分析。
- 远程车型市场、云同步或配置签名。
- 浏览器完整 DCTP 会话。

后续参数方案和实验档案应记录 `vehicle.id`、配置内容哈希、Manifest 设备 ID/固件版本和 Storage Generation，从而保证结果可追溯；这不在当前实现范围内。
