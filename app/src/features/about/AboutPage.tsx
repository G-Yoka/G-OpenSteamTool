import { ExternalLink, Github } from "lucide-react";
import type { AppController } from "../../app/useAppController";
import { APP_VERSION } from "../../app/model";
import appIcon from "../../assets/app-icon.png";
import { Badge, Card, CardHead, KV, KVRow } from "../../shared/ui";

const PROJECT_URL = "https://github.com/G-Yoka/G-OpenSteamTool";

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

      <div className="split-2">
        <Card>
          <CardHead title="设计原则" sub="安全管理 · 配置隔离 · 本地优先" />
          <KV>
            <KVRow label="安全管理"><span className="dim">按哈希识别托管 DLL，避免误删用户已有文件</span></KVRow>
            <KVRow label="配置隔离"><span className="dim">只管理 G-*.lua，不主动改写普通 Lua</span></KVRow>
            <KVRow label="原子写入"><span className="dim">TOML、Lua 与清单走临时文件 + 回滚</span></KVRow>
            <KVRow label="本地优先"><span className="dim">Depot Key 只留在本地配置，不写入日志</span></KVRow>
          </KV>
        </Card>

        <Card>
          <CardHead title="项目" sub="非官方社区项目，与 Valve / Steam 无隶属关系" />
          <KV>
            <KVRow label="许可证"><Badge tone="idle">见仓库</Badge></KVRow>
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
    </div>
  );
}
