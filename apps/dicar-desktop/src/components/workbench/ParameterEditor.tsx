import type { ParameterSnapshot } from "../../domain/types";
import { Alert } from "../ui/alert";
import { EncoderCalibrationPanel } from "./EncoderCalibrationPanel";
import { TypedParameterControl } from "./TypedParameterControl";

export function ParameterEditor({ group, record, records }: { group: string; record: ParameterSnapshot | null; records: ParameterSnapshot[] }) {
  if (group === "编码器与车轮") return <section className="min-w-0"><header className="mb-3"><h2 className="m-0 text-sm">编码器与车轮</h2><p className="m-0 mt-1 text-[11px] text-(--text-muted)">PPR、倍频和有效 CPR 分开呈现；CPR 始终只读计算。</p></header><EncoderCalibrationPanel records={records.filter((item) => item.group === group)} /></section>;
  if (!record) return <Alert>当前分组没有匹配参数，请清除筛选条件。</Alert>;
  return <section className="min-w-0"><header className="mb-3"><h2 className="m-0 text-sm">参数编辑器</h2><p className="m-0 mt-1 text-[11px] text-(--text-muted)">设备 ACK 是 RAM 真值；Flash 只在固化成功后更新。</p></header><TypedParameterControl record={record} /></section>;
}
