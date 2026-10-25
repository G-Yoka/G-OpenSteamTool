//! 命令行自检：`cargo run --example probe_manifest -- "D:\\Steam" [appid]`
//! 与设置页「下载通道自检」走同一段逻辑，便于在无 GUI 环境下排查。

fn main() {
    let mut args = std::env::args().skip(1);
    let steam_dir = args.next().unwrap_or_else(|| r"D:\Steam".to_string());
    let appid = args.next().and_then(|value| value.parse::<u32>().ok());

    match g_opensteamtool::manager::probe_manifest_channel(&steam_dir, appid) {
        Ok(report) => match serde_json::to_string_pretty(&report) {
            Ok(text) => println!("{text}"),
            Err(err) => {
                eprintln!("serialize failed: {err}");
                std::process::exit(1);
            }
        },
        Err(err) => {
            eprintln!("probe failed: {err}");
            std::process::exit(1);
        }
    }
}
