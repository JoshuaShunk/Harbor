#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // Fix PATH for GUI apps so spawned processes can find npx, node, etc.
    let _ = fix_path_env::fix();
    harbor_desktop_lib::run();
}
