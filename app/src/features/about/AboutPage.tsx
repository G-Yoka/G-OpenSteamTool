import { ExternalLink, Github, Info, Sparkles } from "lucide-react";
import type { AppController } from "../../app/useAppController";
import appIcon from "../../assets/app-icon.png";
import { Card, CardHeader } from "../../shared/ui";

const PROJECT_URL = "https://github.com/G-Yoka/G-OpenSteamTool";
export function AboutPage({ controller: _controller }: { controller: AppController }) {
  return <div className="about-layout"><Card className="about-hero"><div className="about-logo"><img src={appIcon} alt="G-OpenSteamTool 图标" /></div><div><span className="status-chip ready"><Sparkles size={14} />Desktop Manager</span><h2>G-OpenSteamTool</h2><p>专注于 Steam 环境、OpenSteamTool 组件和游戏 Lua 配置的 Windows 桌面管理器。</p><div className="about-meta"><span>版本 <b>0.3.0</b></span><span>Tauri 2</span><span>React + Rust</span></div><a className="button" href={PROJECT_URL} target="_blank" rel="noreferrer"><Github size={17} />GitHub 项目页<ExternalLink size={14} /></a></div></Card>
    <Card><CardHeader icon={Info} title="设计原则" /><div className="principle-grid"><div><strong>安全管理</strong><p>通过哈希识别托管 DLL，避免误删用户已有文件。</p></div><div><strong>配置隔离</strong><p>只管理 G-*.lua 文件，不主动修改普通 Lua。</p></div><div><strong>本地优先</strong><p>核心配置与日志都保留在用户自己的 Steam 目录。</p></div></div></Card></div>;
}
