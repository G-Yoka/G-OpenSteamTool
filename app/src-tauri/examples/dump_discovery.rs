//! 命令行自检：`cargo run --example dump_discovery -- "D:\\Steam" [appid] [--ownership]`
//! 与「游戏配置 → 本地自动获取」共用 `discover_depots_from_steam`，便于在没有 GUI 的
//! 环境确认 DLC 枚举与本地 Key 合并结果；加 `--ownership` 则输出编辑器里「主内容 / DLC」
//! 的逐行归属。
fn main() {
    let mut args = std::env::args().skip(1);
    let steam_dir = args.next().unwrap_or_else(|| r"D:\Steam".to_string());
    let appid = args
        .next()
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(1446780);
    let want_ownership = args.any(|flag| flag == "--ownership");

    if want_ownership {
        match g_opensteamtool::manager::depot_ownership_from_steam(&steam_dir, appid) {
            Ok(rows) => match serde_json::to_string_pretty(&rows) {
                Ok(text) => println!("{text}"),
                Err(err) => {
                    eprintln!("serialize failed: {err}");
                    std::process::exit(1);
                }
            },
            Err(err) => {
                eprintln!("ownership failed: {err}");
                std::process::exit(1);
            }
        }
        return;
    }

    match g_opensteamtool::manager::discover_depots_from_steam(&steam_dir, appid) {
        Ok(report) => match serde_json::to_string_pretty(&report) {
            Ok(text) => println!("{text}"),
            Err(err) => {
                eprintln!("serialize failed: {err}");
                std::process::exit(1);
            }
        },
        Err(err) => {
            eprintln!("discovery failed: {err}");
            std::process::exit(1);
        }
    }
}
