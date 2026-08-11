export type ChannelStyle = { color: string; dash: readonly number[]; label: string };

const palette = ["#38bdf8", "#34d399", "#fbbf24", "#fb7185", "#a78bfa", "#22d3ee", "#f472b6", "#a3e635"];
const dashes = [[], [8, 4], [3, 3], [10, 3, 2, 3]] as const;

export function channelStyle(slot: number): ChannelStyle {
  return { color: palette[slot % palette.length], dash: dashes[Math.floor(slot / palette.length) % dashes.length], label: `通道样式 ${slot + 1}` };
}
