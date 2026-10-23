/** 日志行解析：`[时间] [等级] 消息`（方括号与大小写都可缺省）。 */
export type ParsedLogLine = { raw: string; time: string; level: string; message: string };

export function parseLine(raw: string): ParsedLogLine {
  const match = raw.match(/^\[?([^\]]*\d{2}:\d{2}:\d{2}[^\]]*)\]?\s*\[?(trace|debug|info|warn|warning|error)\]?\s*[:\-]?\s*(.*)$/i);
  return {
    raw,
    time: match?.[1]?.match(/\d{2}:\d{2}:\d{2}/)?.[0] ?? "",
    level: (match?.[2] ?? "").toLowerCase().replace("warning", "warn"),
    message: match?.[3] ?? raw,
  };
}

export function formatBytes(bytes: number) {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}
