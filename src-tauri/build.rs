fn main() {
    // 图标是**构建期**嵌进 exe 的资源（Windows 上由 tauri-winres 生成 resource.lib，
    // 运行期窗口图标再从 exe 自己的 RT_GROUP_ICON 取）。而 tauri-build 只声明了
    // tauri.conf.json 与 capabilities 为受跟踪输入 —— 不补这一条，只改图标不会让构建脚本
    // 重跑，重建会沿用上一次的 resource.lib，任务栏与标题栏仍是旧图标（实测过）。
    println!("cargo:rerun-if-changed=icons");
    tauri_build::build()
}
