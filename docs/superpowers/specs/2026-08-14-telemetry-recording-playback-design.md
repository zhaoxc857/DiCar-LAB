# DiCar Tune 波形记录与回放设计

## 1. 目标与边界

0.2.0 增加本机波形记录库、时间轴回放、JSON 导入导出和 CSV 导出。记录器从唯一 Bridge 事件扇出直接接收完整 `UiTelemetryBatch`，不读取 60 秒绘图缓冲，也不保存降采样结果。

首版单次最多 5 分钟，最多 20 条记录、逻辑容量 256 MiB。达到上限自动删除最旧的完整记录。记录失败时整条丢弃，不保留部分数据。

## 2. 数据模型与 IndexedDB

数据库名 `dicar-tune-recordings`，版本 1：

- `recordings`：以 `id` 为 key，保存 `TelemetryRecordingMetadata`；
- `recordingChunks`：以 `[recordingId, chunkIndex]` 为复合 key，并建立 `recordingId` 索引。

元数据包含 schemaVersion、状态、名称/备注、创建/结束时间、停止原因、设备 ID、固件版本、车型 ID、存储 Generation、连接标识、订阅、通道描述、RAM 参数值与 Revision、录制期间新增标记、批次数、点数、丢样数、首末时间戳、块数和逻辑字节数。

`TelemetryRecordingChunk` 保存按到达顺序排列的原始 `UiTelemetryBatch[]`。累计跨度达到 1 秒或点数达到 4096 即刷新一个块。块的 UTF-8 JSON 长度计入逻辑容量。

元数据先以 `recording` 状态创建，正常停止后改为 `complete`。打开数据库时删除所有非 complete 元数据及其块。任一写入失败都停止接收并删除本次元数据和全部块；删除失败仍报告错误并禁止继续录制。

自动清理按 `createdAtMs` 从旧到新删除 complete 记录，保护当前正在回放或导出的 ID。单个导入大于 256 MiB 时直接拒绝。

## 3. 录制状态机

开始要求 AppSnapshot 为 ready、activeSubscription 非空且未暂停，并提供 trim 后 1–64 字符名称与最多 256 字符备注。开始时冻结设备、车型、订阅、通道与参数快照。

状态为 `idle -> starting -> recording -> stopping -> idle`，错误进入 `failed` 后完成清理再回 idle。以下事件正常封存：手动停止、300 秒到期、暂停、断线、活动订阅版本变化。订阅 UI 在调用 Bridge 前先停止；事件监听负责兜底外部订阅变化。

录制控制器与 IndexedDB 写入采用单一 Promise 队列，保证批次、flush、停止和删除严格有序。运行中新增的 marker 从 snapshotChanged 的 marker 后缀收集。

## 4. 导入导出

JSON 文件标识 `format: "dicar-telemetry-recording"`、`schemaVersion: 1`，包含一份完整元数据与按序 chunks。导出文件名为 `dicar-recording-<safe-name>.json`，通过分块 Blob 生成，不先拼接一个超大字符串。

导入先检查文件大小，再解析并全量验证：格式、schema、complete 状态、记录时长、ID/名称、描述符唯一性、订阅一致性、参数值、时间戳有限且非递减、点通道/类型匹配，以及统计可重新计算。验证后生成新 UUID，并在单个 IndexedDB 事务中写入；任何失败零写入。

CSV 为 UTF-8 BOM 宽表，每行一个 sampleSequence/timestamp 组合，列为 `batch_index,subscription_version,dropped_before,timestamp_us,sample_sequence` 加各通道 machine name。`dropped_before` 只在批次首样本写入。所有文本按 RFC 4180 引号规则处理，并对 `= + - @` 起始值前置单引号。

## 5. 回放与 UI

工作台标题增加“波形记录”入口，实时波形工具栏增加开始/停止录制。开始时弹出名称/备注表单。记录管理器按新到旧显示时长、通道、点数、丢样、大小与停止原因，并提供回放、JSON、CSV、删除和 JSON 导入。

回放把所选记录加载到独立、按记录容量构建的 `TelemetryRingBuffer`。实时 workspace store 继续接收数据，但回放不读取或写入它，也不调用任何 DesktopBridge 方法。

`WaveformCanvas` 增加可选 `viewportEndUs` 和 aria label，以复用降采样、Y 轴、A/B 游标和数据表。回放支持播放/暂停、拖动、单步、0.25x/0.5x/1x/2x/4x；到末尾自动暂停。关闭回放后实时画面保持原状态。

## 6. 验证

使用 `fake-indexeddb 6.2.5` 测试库迁移、分块、限制、清理、写失败整条删除、导入原子性和往返。纯函数测试 JSON/CSV 校验与转义。组件测试覆盖状态与零 Bridge 命令；Playwright 使用 Mock 完成录制、订阅变化封存、记录库回放与下载，不等待真实 5 分钟。
