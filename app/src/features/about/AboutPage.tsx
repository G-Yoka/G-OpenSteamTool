import { ExternalLink, Github } from "lucide-react";
import type { AppController } from "../../app/useAppController";
import { APP_VERSION } from "../../app/model";
import appIcon from "../../assets/app-icon.png";
import { Card, CardHead, KV, KVRow } from "../../shared/ui";

const PROJECT_URL = "https://github.com/G-Yoka/G-OpenSteamTool";
const LICENSE_URL = `${PROJECT_URL}/blob/main/LICENSE`;

export function AboutPage({ controller: _controller }: { controller: AppController }) {
  return (
    <div className="page-full">
      <Card>
        <div className="about-hero">
          <img className="about-logo" src={appIcon} alt="G-OpenSteamTool 图标" />
          <div>
            <h2>G-OpenSteamTool</h2>
            <p className="lede">专注于 Steam 环境、OpenSteamTool 组件与游戏 Lua 配置的 Windows 桌面管理器。所有配置与日志都留在你自己的 Steam 目录里。</p>
            <div className="meta-row">
              <span className="chip">v{APP_VERSION}</span>
              <span className="chip">Tauri 2</span>
              <span className="chip">React + Rust</span>
              <span className="chip">Windows 10/11 x64</span>
            </div>
          </div>
        </div>
      </Card>

      <Card>
        <CardHead title="项目" sub="非官方社区项目，与 Valve / Steam 无隶属关系" />
        <KV>
          <KVRow label="许可证">
            <a href={LICENSE_URL} target="_blank" rel="noreferrer" title="在 GitHub 上查看许可证">
              <code>{LICENSE_URL}</code>
            </a>
            <ExternalLink size={11} />
          </KVRow>
          <KVRow label="更新方式"><span className="dim">手动从 Releases 下载覆盖</span></KVRow>
          <KVRow label="界面版本"><code>UI v{APP_VERSION}</code></KVRow>
        </KV>
        <div className="btn-row spaced">
          <a className="btn btn-primary" href={PROJECT_URL} target="_blank" rel="noreferrer">
            <Github size={12} />GitHub 项目页<ExternalLink size={11} />
          </a>
          <a className="btn" href={`${PROJECT_URL}/releases/latest`} target="_blank" rel="noreferrer">
            最新发布<ExternalLink size={11} />
          </a>
        </div>
      </Card>
    </div>
  );
}
