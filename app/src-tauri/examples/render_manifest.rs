//! 打印生成的 manifest.lua（排查格式问题）：`cargo run --example render_manifest`

fn main() {
    let mut settings = g_opensteamtool::manager::ManagerSettings::default();
    if let Some(endpoint) = std::env::args().nth(1) {
        settings.manifest_lua_endpoint = endpoint;
    }
    print!("{}", g_opensteamtool::manager::render_manifest_lua(&settings));
}
