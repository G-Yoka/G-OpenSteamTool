import type { LucideIcon } from "lucide-react";

export type Command = {
  id: string;
  title: string;
  group: "跳转" | "环境" | "组件" | "数据";
  icon: LucideIcon;
  hint?: string;
  keywords?: string;
  run: () => void;
  disabled?: boolean;
};

/**
 * 子序列打分：连续命中得分更高，整词命中直接置顶。
 * 返回 -1 表示不匹配。够用就好——命令面板只有几十条，不需要真正的模糊排序库。
 */
export function fuzzyScore(query: string, target: string): number {
  if (!query) return 0;
  const q = query.trim().toLowerCase();
  if (!q) return 0;
  const text = target.toLowerCase();
  const direct = text.indexOf(q);
  if (direct >= 0) return 200 - direct;
  let cursor = 0;
  let score = 0;
  let streak = 0;
  for (const char of q) {
    const hit = text.indexOf(char, cursor);
    if (hit < 0) return -1;
    streak = hit === cursor ? streak + 1 : 0;
    score += 1 + streak;
    cursor = hit + 1;
  }
  return score;
}

export function rankCommands(commands: Command[], query: string): Command[] {
  if (!query.trim()) return commands;
  return commands
    .map((command) => ({ command, score: fuzzyScore(query, `${command.title} ${command.keywords ?? ""} ${command.group}`) }))
    .filter((entry) => entry.score >= 0)
    .sort((a, b) => b.score - a.score)
    .map((entry) => entry.command);
}
