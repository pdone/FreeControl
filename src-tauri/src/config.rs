//! 配置持久化层（design D5 / D7；tasks 5.1–5.4）。
//!
//! 单一配置文件 `%APPDATA%/FreeControl2/config.json`，含 `schemaVersion` 字段。
//! 内容：设备列表（别名 ⇄ 连接标识）、默认参数、设备覆盖及其启用/禁用开关、下载通道偏好。
//!
//! ## 初始化（tasks 5.1）
//! 文件不存在时按默认值初始化并立即落盘一次；`schemaVersion` 由此写入（老文件缺失时补默认值）。
//!
//! ## 写入策略（tasks 5.2）
//! 「唯一命名临时文件 + 原子 rename」：见 [`save_config_to`]。
//! 「去抖」不在这一层：由前端在调用 `save_config` 前完成（`store.ts::scheduleSave` / `flushSave`），
//! 后端每次调用都立即落盘 —— 后端自己发起的写入（如代理地址增删、设备增删改）必须即时生效。
//!
//! ## 字段归属
//! 8 组默认参数与设备覆盖 `values` 的字段模型归前端所有，后端把它们当**透明 JSON** 存取
//! （只保证结构不丢、可前向兼容），不在此处复刻一份字段定义。

use crate::AppState;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Manager, State};

/// 配置文件 schema 版本（design D7：预留演进，本版为 1）。
pub const SCHEMA_VERSION: u32 = 1;

/// 设备类型取值（与前端 `DeviceKind` 对齐）。
pub const KIND_USB: &str = "usb";
pub const KIND_WIFI: &str = "wifi";

/// 无线连接的默认端口（用户只填 IP 时使用）。
pub const DEFAULT_WIFI_PORT: u16 = 5555;

/// 设备登记项。连接标识（serial 或 IP:端口）由 kind 决定。
#[derive(Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Device {
    pub id: String,
    pub alias: String,
    /// "usb" | "wifi"
    pub kind: String,
    /// USB 连接的 serial（wifi 时为空）
    pub serial: String,
    /// WiFi 连接的 IP（usb 时为空）
    pub ip: String,
    /// WiFi 连接的端口（默认 5555）
    pub port: u16,
    // 序列化时另输出派生的 `connectionId`（见下方手写 impl），字段本身不在此结构里 ——
    // 它是计算值，存一份就会有漂移的可能。
}

/// 手写序列化：在登记字段之外多输出一个**派生**字段 `connectionId`（tasks 6.1）。
///
/// 连接标识的算法只允许有一份（[`Device::connection_id`]），而界面展示与命令预览
/// （Group 8 的 `-s <连接标识>`）都需要它，故由后端算好一并下发，前端不自己拼
/// 「USB 用 serial / 无线用 IP:端口」。
///
/// 该字段是只读派生值：反序列化时被忽略（`Deserialize` 派生默认不拒绝未知字段），
/// 每次写出时按当前 `kind` / `serial` / `ip` / `port` 重算，因此不会与真实标识不一致；
/// 手工编辑 `config.json` 里的它没有任何效果。
impl Serialize for Device {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut st = serializer.serialize_struct("Device", 7)?;
        st.serialize_field("id", &self.id)?;
        st.serialize_field("alias", &self.alias)?;
        st.serialize_field("kind", &self.kind)?;
        st.serialize_field("serial", &self.serial)?;
        st.serialize_field("ip", &self.ip)?;
        st.serialize_field("port", &self.port)?;
        st.serialize_field("connectionId", &self.connection_id())?;
        st.end()
    }
}

/// 「别名 ⇄ 连接标识」映射中**标识一侧的唯一算法来源**（tasks 5.3）。
///
/// 界面展示、`adb -s`、命令拼装的 `-s`、重复校验都读这里，不各自拼一遍 ——
/// 否则 USB 用 serial、无线用 `IP:端口` 这条规则会散落成多份实现。
impl Device {
    /// 连接标识：USB 用 serial，无线用 `IP:端口`（device-management spec）。
    ///
    /// USB 设备的 serial 在新增时可能为空（用户只知道插了台机器，serial 待探测补全），
    /// 此时返回空串 —— 调用方需按「标识未知」处理，不能拿它去 `adb -s`。
    pub fn connection_id(&self) -> String {
        match self.kind.as_str() {
            KIND_WIFI => format!("{}:{}", self.ip.trim(), self.port),
            _ => self.serial.trim().to_string(),
        }
    }

    /// 规范化用户输入：去首尾空白；无线连接未填端口时取默认 5555。
    fn normalize(&mut self) {
        self.id = self.id.trim().to_string();
        self.alias = self.alias.trim().to_string();
        self.serial = self.serial.trim().to_string();
        self.ip = self.ip.trim().to_string();
        if self.kind == KIND_WIFI && self.port == 0 {
            self.port = DEFAULT_WIFI_PORT;
        }
    }

    /// 校验：类型合法、无线必须有 IP；USB 允许 serial 留空（待探测补全）。
    fn validate(&self) -> Result<(), String> {
        match self.kind.as_str() {
            KIND_USB => Ok(()),
            KIND_WIFI => {
                if self.ip.is_empty() {
                    return Err("无线设备需要填写 IP 地址".to_string());
                }
                if self.port == 0 {
                    return Err("无线设备的端口不合法".to_string());
                }
                Ok(())
            }
            other => Err(format!("设备类型不合法：{}", other)),
        }
    }
}

/// 单台设备的参数覆盖（design D5）：值与开关一并持久化，禁用不清值。
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct OverrideEntry {
    pub enabled: bool,
    pub values: serde_json::Value,
}

/// 出厂默认代理地址（design D9）：**唯一**声明点。
///
/// 它们只是配置文件的**初始内容** —— 首次创建 `config.json` 时写入一次，
/// 此后与用户自行追加的地址完全同等（一律可删，删掉不会自动回来），
/// 不是代码内置项：`download.rs` 不声明任何地址，通道列表只读配置。
pub const DEFAULT_PROXIES: [&str; 2] = ["https://gh-proxy.org", "https://ghproxy.net/"];

fn default_proxies() -> Vec<String> {
    DEFAULT_PROXIES.iter().map(|s| (*s).to_string()).collect()
}

/// 下载通道偏好（design D9）：代理地址列表，顺序即自动降级的顺序。
///
/// 列表里没有「只读项」这一档 —— 界面上唯一的只读行是 GitHub 直连地址，
/// 它不是代理、不可删，也不落在这个列表里（由 `download::ProxyList::direct` 下发）。
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Channels {
    /// 缺字段时用出厂默认值：手工编辑过的配置文件少写这一项不该让通道列表变成空的。
    #[serde(default = "default_proxies")]
    pub proxies: Vec<String>,
}

impl Default for Channels {
    fn default() -> Self {
        Channels {
            proxies: default_proxies(),
        }
    }
}

/// 保留策略取值。配置里存的是这三个**自描述**的字符串（不是天数，也不是「0 = 永久」这类约定）。
pub const RETENTION_7D: &str = "7d";
pub const RETENTION_30D: &str = "30d";
pub const RETENTION_FOREVER: &str = "forever";

/// 出厂默认保留天数（「保留 7 天」）。
pub const DEFAULT_RETENTION_DAYS: u32 = 7;

/// 日志设置（design D5）：目前只有保留策略一项。
///
/// 为什么不用 `retentionDays: 0` 表示「永久」：配置文件是用户可手改的文本，`0` 读起来更像
/// 「不保留」，而 `forever` 不用靠约定去记。**未知取值一律退化为出厂默认** ——
/// 手工改错一个字段不该变成「日志全被删掉」或「再也不清理」。
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Logging {
    /// "7d" | "30d" | "forever"
    #[serde(default = "default_retention")]
    pub retention: String,
}

fn default_retention() -> String {
    RETENTION_7D.to_string()
}

impl Default for Logging {
    fn default() -> Self {
        Logging {
            retention: default_retention(),
        }
    }
}

impl Logging {
    /// 保留天数；`None` 表示永久保留（不清理）。字符串 → 天数的换算只有这一处。
    pub fn retention_days(&self) -> Option<u32> {
        match self.retention.as_str() {
            RETENTION_30D => Some(30),
            RETENTION_FOREVER => None,
            _ => Some(DEFAULT_RETENTION_DAYS),
        }
    }
}

/// 应用配置根对象。
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    /// 缺字段时按 0 读入，再由 [`load_config_from`] 补成当前 [`SCHEMA_VERSION`]
    /// —— 手工编辑过的配置文件少写一个字段不该让整个应用读不出配置。
    #[serde(default)]
    pub schema_version: u32,
    pub devices: Vec<Device>,
    /// 8 组默认参数（前端拥有字段模型；后端以 JSON 透明存储，便于前向兼容）。
    #[serde(default)]
    pub defaults: serde_json::Value,
    /// 设备覆盖表：`deviceId -> OverrideEntry`。
    #[serde(default)]
    pub overrides: HashMap<String, OverrideEntry>,
    /// 下载通道偏好。
    #[serde(default)]
    pub channels: Channels,
    /// 日志设置（保留策略）。缺字段时取出厂默认，老配置文件照常可读。
    #[serde(default)]
    pub logging: Logging,
}

/// 兜底默认值与 [`AppConfig::fresh`] 保持一致：`main.rs` 在配置真的读不出来时
/// 用 `unwrap_or_default()` 起一个可用状态，不该在那里拿到 `schemaVersion = 0`。
impl Default for AppConfig {
    fn default() -> Self {
        AppConfig::fresh()
    }
}

impl AppConfig {
    pub fn fresh() -> Self {
        AppConfig {
            schema_version: SCHEMA_VERSION,
            devices: Vec::new(),
            defaults: serde_json::Value::Null,
            overrides: HashMap::new(),
            channels: Channels::default(),
            logging: Logging::default(),
        }
    }

    /// 新增或更新一台设备，返回其 id（tasks 5.3）。
    ///
    /// - `id` 为空 → 新增（后端分配 id，前端不需要自己造）；
    /// - `id` 非空 → 更新既有设备；找不到即报错（不静默变成新增，否则前端 id 传错会凭空多一台）。
    ///
    /// 校验先于修改：出错时调用方持有的配置不会处于半改状态。
    pub fn upsert_device(&mut self, device: Device) -> Result<String, String> {
        let mut device = device;
        device.normalize();
        device.validate()?;

        // 连接标识（能确定时）必须唯一：同一条 serial / IP:端口 登记两次只会互相打架。
        // 标识未知（USB 未探测到 serial）时不去重，否则多台待识别设备无法并存。
        let conn = device.connection_id();
        if !conn.is_empty()
            && self
                .devices
                .iter()
                .any(|d| d.id != device.id && d.connection_id() == conn)
        {
            return Err(format!("连接标识 {} 已被其他设备使用", conn));
        }

        if device.id.is_empty() {
            device.id = new_device_id();
            let id = device.id.clone();
            self.devices.push(device);
            return Ok(id);
        }

        let idx = self
            .devices
            .iter()
            .position(|d| d.id == device.id)
            .ok_or_else(|| "设备不存在".to_string())?;
        let id = device.id.clone();
        self.devices[idx] = device;
        Ok(id)
    }

    /// 删除设备：别名映射、参数覆盖与覆盖启停开关一并清除（device-management spec）。
    ///
    /// 覆盖表必须一起删：留下孤儿覆盖后，设备 id 万一被复用，旧覆盖会毫无征兆地生效。
    /// 返回该设备是否存在（供命令层区分「删了」与「本来就没有」）。
    pub fn remove_device(&mut self, id: &str) -> bool {
        let before = self.devices.len();
        self.devices.retain(|d| d.id != id);
        self.overrides.remove(id);
        self.devices.len() != before
    }
}

/// 新设备 id：时间戳 + 进程内序号，不为此引入 uuid 依赖（同一毫秒内连续新增也不会撞）。
fn new_device_id() -> String {
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    format!("dev-{}-{}", ms, SEQ.fetch_add(1, Ordering::Relaxed))
}

/// 应用数据目录名。spec 约定为 `%APPDATA%/FreeControl2`（不随 bundle identifier 变化）。
pub const APP_DIR_NAME: &str = "FreeControl2";

/// 应用数据根目录：`%APPDATA%/FreeControl2`（design D7）。
///
/// 这里是**唯一**的数据根解析点：`config.json` 与 scrcpy 运行环境目录都挂在它下面，
/// 二者不会各自算出一个目录（tasks 3.1 要求「与配置的应用数据目录一致」）。
///
/// 不用 Tauri 的 `app_data_dir()`：它把 bundle identifier 当目录名用
/// （本工程即 `%APPDATA%/com.freecontrol2`），与 spec 约定的目录名不符；
/// 改 identifier 也会顺带搬走用户数据。故以 `data_dir()`（Windows 下即 `%APPDATA%`）
/// 为基座拼固定目录名。
pub fn app_data_root(app: &AppHandle) -> Result<PathBuf, String> {
    let base = app.path().data_dir().map_err(|e| e.to_string())?;
    Ok(base.join(APP_DIR_NAME))
}

/// 配置文件在给定数据根下的路径（纯函数，便于用临时目录验证初始化与读写，tasks 5.1）。
pub fn config_path_in(root: &Path) -> PathBuf {
    root.join("config.json")
}

fn config_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(config_path_in(&app_data_root(app)?))
}

/// 从磁盘加载配置（应用数据根下的 `config.json`）。
pub fn load_config(app: &AppHandle) -> Result<AppConfig, String> {
    load_config_from(&config_path(app)?)
}

/// 从指定路径加载配置；文件不存在时按默认值初始化并立即落盘一次
/// （tasks 5.1 / design D7 Migration Plan）。
///
/// 8 组默认参数不在这里填：字段模型归前端所有（`src/lib/defaults.ts`），后端把 `defaults`
/// 当透明 JSON 存取；前端拿到空值后立即回写内置默认（`store.ts::initApp`），
/// 因此首次启动结束时盘上存的是完整默认参数，而不是一个空的 `null`。
/// 写坏 / 不兼容时的恢复方式也保持简单：删掉 `config.json` 即回到出厂默认。
pub fn load_config_from(path: &Path) -> Result<AppConfig, String> {
    if !path.exists() {
        let cfg = AppConfig::fresh();
        save_config_to(path, &cfg)?;
        return Ok(cfg);
    }
    let raw = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let mut cfg: AppConfig = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
    // 兼容：老文件缺 schemaVersion 时补默认值。
    if cfg.schema_version == 0 {
        cfg.schema_version = SCHEMA_VERSION;
    }
    Ok(cfg)
}

/// 应用数据根下的原子写入（命令层入口）。
///
/// 所有配置写入都走这里（前端保存、设备增删改、代理地址增删），因此写失败的日志也只在这里记一次 ——
/// 「界面说已保存、盘上却没变」这类问题，靠这一行才能定位（log-persistence tasks 5.4）。
pub fn save_config_to_disk(app: &AppHandle, cfg: &AppConfig) -> Result<(), String> {
    let result = config_path(app).and_then(|path| save_config_to(&path, cfg));
    if let Err(e) = &result {
        crate::logger::error("config", format!("写入配置失败：{e}"));
    }
    result
}

/// 原子写入（tasks 5.2）：写**唯一命名**的临时文件 → `sync_all` → `rename` 覆盖目标。
///
/// 两条性质来自「临时文件 + 原子 rename」（同目录内的 rename 在 Windows 上是原子的）：
/// - 任意时刻中断（崩溃 / 强杀 / 写盘失败）都不会留下半损坏的 `config.json` ——
///   目标文件要么是旧的完整内容、要么是新的完整内容；
/// - 临时文件名带 pid / 时间戳 / 序号，因此并发的两次写入互不截断。共用一个临时名时，
///   后写者会把先写者写了一半的文件截断，rename 出去就是半损坏配置。
///
/// 失败路径会把临时文件删掉，不留垃圾（进程被杀留下的残留由 [`cleanup_stale_tmp`] 在启动时清）。
pub fn save_config_to(path: &Path, cfg: &AppConfig) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let bytes = serde_json::to_vec_pretty(cfg).map_err(|e| e.to_string())?;
    let tmp = temp_path_for(path);
    {
        let mut f = fs::File::create(&tmp).map_err(|e| e.to_string())?;
        if let Err(e) = f.write_all(&bytes).and_then(|_| f.sync_all()) {
            drop(f); // Windows 上先关句柄才删得掉
            let _ = fs::remove_file(&tmp);
            return Err(e.to_string());
        }
    }
    if let Err(e) = replace_file(&tmp, path) {
        let _ = fs::remove_file(&tmp);
        return Err(e);
    }
    Ok(())
}

/// 用临时文件替换目标文件。
///
/// Windows 上目标可能被短暂打开（杀软 / 索引服务 / 并发读取），rename 会瞬时失败，
/// 故重试几次而不是把一次保存直接判失败 —— 配置写入失败等于用户的改动被悄悄丢掉。
fn replace_file(from: &Path, to: &Path) -> Result<(), String> {
    let mut last = String::new();
    for attempt in 0..RENAME_RETRIES {
        match fs::rename(from, to) {
            Ok(()) => return Ok(()),
            Err(e) => {
                last = e.to_string();
                if attempt + 1 < RENAME_RETRIES {
                    std::thread::sleep(std::time::Duration::from_millis(20));
                }
            }
        }
    }
    Err(last)
}

/// 写配置的临时文件名前缀（清理残留时据此识别）。
pub const TMP_PREFIX: &str = "config.json.";

/// rename 重试次数。
const RENAME_RETRIES: u32 = 3;

/// 临时文件路径：`config.json.<pid>-<nanos>-<seq>.tmp`（与目标同目录，保证 rename 不跨卷）。
fn temp_path_for(path: &Path) -> PathBuf {
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "config.json".to_string());
    path.with_file_name(format!("{}.{}-{}-{}.tmp", name, std::process::id(), nanos, seq))
}

/// 启动时清理上次运行残留的临时文件（tasks 5.2：被强杀后不留垃圾）。
///
/// 只删 `config.json.<...>.tmp` 形状的文件，配置文件本身与其它文件不受影响。
pub fn cleanup_stale_tmp(root: &Path) {
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with(TMP_PREFIX) && name.ends_with(".tmp") {
            let _ = fs::remove_file(entry.path());
        }
    }
}

// ── Tauri 命令 ─────────────────────────────────────────────

#[tauri::command]
pub fn get_config(state: State<'_, AppState>) -> AppConfig {
    state.config.lock().unwrap().clone()
}

#[tauri::command]
pub fn save_config(
    app: AppHandle,
    state: State<'_, AppState>,
    config: AppConfig,
) -> Result<(), String> {
    // 先落盘再改内存：写失败时内存保持原样，界面上的「已保存」才不会说谎
    // （与 `device` 模块的写命令保持同一套语义）。
    let retention = config.logging.retention_days();
    save_config_to_disk(&app, &config)?;
    *state.config.lock().unwrap() = config;
    // 日志保留策略随配置一起落盘，改完立即按新策略清理一次（tasks 4.2 / design D7）：
    // 放在这里而不是另开一条命令，是为了让前端不需要第二条保存通道，
    // 也就不会出现「界面改了、后端不知道」的空档。值没变时 `set_retention` 自己会跳过。
    crate::logger::set_retention(retention);
    Ok(())
}

#[cfg(test)]
mod tests {
    //! 配置持久化的可验证部分（tasks 5.1–5.4）。
    //!
    //! 路径与读写都以「数据根目录」为参数（`config_path_in` / `load_config_from` /
    //! `save_config_to`），因此这些用例全程只碰临时目录，不会动用户真实的 `config.json`。

    use super::*;
    use std::sync::Arc;
    use std::thread;

    fn unique_temp_dir(tag: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "fcn-config-{}-{}-{}",
            tag,
            std::process::id(),
            nanos
        ))
    }

    fn usb(id: &str, alias: &str, serial: &str) -> Device {
        Device {
            id: id.to_string(),
            alias: alias.to_string(),
            kind: KIND_USB.to_string(),
            serial: serial.to_string(),
            ip: String::new(),
            port: 0,
        }
    }

    fn wifi(id: &str, alias: &str, ip: &str, port: u16) -> Device {
        Device {
            id: id.to_string(),
            alias: alias.to_string(),
            kind: KIND_WIFI.to_string(),
            serial: String::new(),
            ip: ip.to_string(),
            port,
        }
    }

    fn leftover_tmp_files(dir: &Path) -> Vec<String> {
        fs::read_dir(dir)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|n| n.ends_with(".tmp"))
            .collect()
    }

    // ── tasks 5.1：初始化与 schemaVersion ─────────────────────

    #[test]
    fn missing_file_is_initialized_and_written() {
        let dir = unique_temp_dir("init");
        let path = config_path_in(&dir);
        assert!(!path.exists());

        let cfg = load_config_from(&path).unwrap();

        // 文件不存在 → 按默认值初始化并落盘一次。
        assert!(path.exists(), "配置文件应被创建");
        assert_eq!(cfg.schema_version, SCHEMA_VERSION);
        assert!(cfg.devices.is_empty());
        assert!(cfg.overrides.is_empty());
        // 出厂默认代理地址随首次创建写入（design D9）：两条、顺序固定。
        assert_eq!(cfg.channels.proxies, DEFAULT_PROXIES.to_vec());

        // 盘上确实带 schemaVersion 字段（而不是只在内存里）。
        let raw = fs::read_to_string(&path).unwrap();
        assert!(raw.contains("\"schemaVersion\""));
        assert!(raw.contains(&SCHEMA_VERSION.to_string()));

        // 再读一次读到的是同一份默认值（不是反复初始化）。
        assert_eq!(load_config_from(&path).unwrap().schema_version, SCHEMA_VERSION);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_schema_version_is_filled_on_load() {
        let dir = unique_temp_dir("schema");
        fs::create_dir_all(&dir).unwrap();
        let path = config_path_in(&dir);
        // 老文件（无 schemaVersion）：读进来时补上默认值，不因缺字段而失败。
        fs::write(
            &path,
            br#"{"devices":[],"defaults":null,"overrides":{},"channels":{"proxies":[]}}"#,
        )
        .unwrap();

        let cfg = load_config_from(&path).unwrap();

        assert_eq!(cfg.schema_version, SCHEMA_VERSION);
        let _ = fs::remove_dir_all(&dir);
    }

    // ── 日志保留策略（log-persistence tasks 4.1）────────────────

    #[test]
    fn logging_defaults_to_seven_days_when_field_absent() {
        let dir = unique_temp_dir("logging-default");
        fs::create_dir_all(&dir).unwrap();
        let path = config_path_in(&dir);
        // 老文件（无 logging 字段）：按出厂默认 7 天处理，其余字段照常读出。
        fs::write(
            &path,
            br#"{"devices":[],"defaults":null,"overrides":{},"channels":{"proxies":["https://x.example/"]}}"#,
        )
        .unwrap();

        let cfg = load_config_from(&path).unwrap();

        assert_eq!(cfg.logging.retention, RETENTION_7D);
        assert_eq!(cfg.logging.retention_days(), Some(DEFAULT_RETENTION_DAYS));
        assert_eq!(cfg.channels.proxies, vec!["https://x.example/".to_string()]);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn logging_retention_maps_values_and_unknown_falls_back() {
        let days = |v: &str| Logging {
            retention: v.to_string(),
        }
        .retention_days();

        assert_eq!(days(RETENTION_7D), Some(7));
        assert_eq!(days(RETENTION_30D), Some(30));
        assert_eq!(days(RETENTION_FOREVER), None);
        // 手工改错一个字段不该变成「日志全被删掉」或「再也不清理」
        assert_eq!(days("3d"), Some(DEFAULT_RETENTION_DAYS));
        assert_eq!(days(""), Some(DEFAULT_RETENTION_DAYS));
        assert_eq!(Logging::default().retention_days(), Some(DEFAULT_RETENTION_DAYS));
    }

    #[test]
    fn logging_retention_round_trips_through_the_file() {
        let dir = unique_temp_dir("logging-round");
        let path = config_path_in(&dir);
        let mut cfg = load_config_from(&path).unwrap();
        cfg.logging.retention = RETENTION_FOREVER.to_string();
        save_config_to(&path, &cfg).unwrap();

        assert_eq!(load_config_from(&path).unwrap().logging.retention_days(), None);
        let _ = fs::remove_dir_all(&dir);
    }

    // ── tasks 5.2：去抖 + 原子替换 ──────────────────────────────

    #[test]
    fn concurrent_saves_never_leave_a_corrupted_file() {
        let dir = unique_temp_dir("atomic");
        let path = Arc::new(config_path_in(&dir));

        // 并发写：唯一临时名保证任何一次 rename 出去的都是某次完整写入的结果。
        let mut handles = Vec::new();
        for i in 0..8 {
            let path = Arc::clone(&path);
            handles.push(thread::spawn(move || {
                let mut cfg = AppConfig::fresh();
                cfg.devices
                    .push(usb(&format!("dev-{}", i), &format!("设备{}", i), &format!("SERIAL{}", i)));
                // 目标文件可能被另一次 rename 短暂占用而失败，属预期；关键是不产生损坏内容。
                let _ = save_config_to(&path, &cfg);
            }));
        }
        for h in handles {
            h.join().unwrap();
        }

        // 最终文件始终可解析，且恰好是「一台设备」的完整内容（不是两次写入的混合体）。
        let cfg = load_config_from(&path).unwrap();
        assert_eq!(cfg.devices.len(), 1);
        assert!(cfg.devices[0].alias.starts_with("设备"));
        assert!(cfg.devices[0].serial.starts_with("SERIAL"));
        assert!(leftover_tmp_files(&dir).is_empty(), "不应残留临时文件");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn stale_tmp_files_are_cleaned_without_touching_config() {
        let dir = unique_temp_dir("stale");
        fs::create_dir_all(&dir).unwrap();
        let path = config_path_in(&dir);
        save_config_to(&path, &AppConfig::fresh()).unwrap();
        // 模拟上次运行被强杀：留下一个写了一半的临时文件。
        let stale = dir.join(format!("{}1-2-3.tmp", TMP_PREFIX));
        fs::write(&stale, b"{ half written").unwrap();

        cleanup_stale_tmp(&dir);

        assert!(!stale.exists(), "残留临时文件应被清掉");
        assert!(path.exists(), "配置文件本身不应被删");
        // 半成品不会被当成配置读进来。
        assert_eq!(load_config_from(&path).unwrap().schema_version, SCHEMA_VERSION);

        let _ = fs::remove_dir_all(&dir);
    }

    // ── tasks 5.3：设备列表的读写持久化 ────────────────────────

    #[test]
    fn connection_id_follows_device_kind() {
        assert_eq!(usb("", "", "ABC123").connection_id(), "ABC123");
        assert_eq!(wifi("", "", "192.168.1.5", 5555).connection_id(), "192.168.1.5:5555");
    }

    #[test]
    fn serialized_device_carries_derived_connection_id() {
        // 前端展示与命令预览都用后端下发的 `connectionId`（tasks 6.1），
        // 它是派生值：写出时重算，改 serial 后立刻跟着变。
        let wifi = wifi("dev-1", "平板", " 192.168.1.7 ", 0);
        let json = serde_json::to_value(&wifi).unwrap();
        assert_eq!(json["connectionId"], "192.168.1.7:0");
        assert_eq!(json["kind"], "wifi");

        let usb_json = serde_json::to_value(usb("dev-2", "主力机", "ABC123")).unwrap();
        assert_eq!(usb_json["connectionId"], "ABC123");
        // serial 未知（待探测补全）时标识为空串，前端据此显示「待检测」。
        let pending = serde_json::to_value(usb("dev-3", "新设备", "")).unwrap();
        assert_eq!(pending["connectionId"], "");

        // 反序列化忽略该字段：前端把整份设备对象回传也不会把它写进配置结构。
        let back: Device =
            serde_json::from_value(serde_json::json!({ "id": "dev-4", "alias": "A", "kind": "usb", "serial": "S1", "ip": "", "port": 0, "connectionId": "被忽略的旧值" }))
                .unwrap();
        assert_eq!(back.connection_id(), "S1");
    }

    #[test]
    fn aliases_and_connection_ids_survive_restart() {
        let dir = unique_temp_dir("devices");
        let path = config_path_in(&dir);

        let mut cfg = load_config_from(&path).unwrap();
        cfg.upsert_device(usb("", "主力机", "ABC123")).unwrap();
        cfg.upsert_device(wifi("", "备用机", "192.168.1.5", 5555)).unwrap();
        save_config_to(&path, &cfg).unwrap();

        // 模拟重启：整份配置重新从磁盘读。
        let reloaded = load_config_from(&path).unwrap();
        let ids: Vec<String> = reloaded.devices.iter().map(|d| d.connection_id()).collect();
        assert_eq!(ids, vec!["ABC123".to_string(), "192.168.1.5:5555".to_string()]);
        assert_eq!(reloaded.devices[0].alias, "主力机");
        assert_eq!(reloaded.devices[0].kind, KIND_USB);
        assert_eq!(reloaded.devices[1].alias, "备用机");
        assert_eq!(reloaded.devices[1].kind, KIND_WIFI);
        // id 由后端分配且落盘，后续补全 serial 时不会换 id（换 id 会让覆盖与别名一起丢）。
        assert!(reloaded.devices[0].id.starts_with("dev-"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn usb_device_may_be_added_before_serial_is_known() {
        // spec 场景：新增 USB 设备时 serial 留空，后续探测到再补全连接标识。
        let mut cfg = AppConfig::fresh();
        let id = cfg.upsert_device(usb("", "新设备", "")).unwrap();
        assert_eq!(cfg.devices.len(), 1);
        assert_eq!(cfg.devices[0].connection_id(), "");

        let mut detected = cfg.devices[0].clone();
        detected.serial = "SERIAL-9".to_string();
        cfg.upsert_device(detected).unwrap();

        assert_eq!(cfg.devices.len(), 1, "补全 serial 不应变成新增");
        assert_eq!(cfg.devices[0].id, id);
        assert_eq!(cfg.devices[0].connection_id(), "SERIAL-9");
    }

    #[test]
    fn duplicate_connection_id_is_rejected() {
        let mut cfg = AppConfig::fresh();
        cfg.upsert_device(usb("", "A", "S1")).unwrap();

        let err = cfg.upsert_device(usb("", "B", "S1")).unwrap_err();

        assert!(err.contains("S1"));
        assert_eq!(cfg.devices.len(), 1);
        assert_eq!(cfg.devices[0].alias, "A", "校验失败时配置不应被修改");
    }

    #[test]
    fn wifi_device_requires_ip_and_fills_default_port() {
        let mut cfg = AppConfig::fresh();

        assert!(cfg.upsert_device(wifi("", "无 IP", "", 0)).is_err());
        assert!(cfg.devices.is_empty());

        // 只填 IP 时端口取默认 5555，前后空白被裁掉。
        cfg.upsert_device(wifi("", "平板", " 192.168.1.7 ", 0)).unwrap();
        assert_eq!(cfg.devices[0].connection_id(), "192.168.1.7:5555");

        // 类型非法直接拒绝（避免出现「USB 走 wifi 分支」这种不一致状态）。
        let mut bad = usb("", "怪设备", "S9");
        bad.kind = "bluetooth".to_string();
        assert!(cfg.upsert_device(bad).is_err());
    }

    #[test]
    fn updating_unknown_device_id_is_rejected() {
        let mut cfg = AppConfig::fresh();
        let err = cfg.upsert_device(usb("dev-404", "幽灵", "S1")).unwrap_err();
        assert!(err.contains("设备不存在"));
        assert!(cfg.devices.is_empty());
    }

    #[test]
    fn removing_device_clears_its_override_and_switch() {
        let mut cfg = AppConfig::fresh();
        let id = cfg.upsert_device(usb("", "A", "S1")).unwrap();
        cfg.overrides.insert(
            id.clone(),
            OverrideEntry {
                enabled: false,
                values: serde_json::json!({ "video": { "bitrate": 4 } }),
            },
        );

        assert!(cfg.remove_device(&id));

        assert!(cfg.devices.is_empty());
        assert!(!cfg.overrides.contains_key(&id), "覆盖值与开关应一并清除");
        // 再删一次：设备本就不存在，返回 false（命令层据此报「设备不存在」）。
        assert!(!cfg.remove_device(&id));
    }

    // ── tasks 5.4：默认参数持久化 ──────────────────────────────

    #[test]
    fn defaults_and_overrides_survive_restart() {
        let dir = unique_temp_dir("defaults");
        let path = config_path_in(&dir);

        let mut cfg = load_config_from(&path).unwrap();
        let id = cfg.upsert_device(usb("", "主力机", "ABC123")).unwrap();
        // `defaults` / 覆盖 `values` 是前端拥有的字段模型，后端只做透明存取。
        cfg.defaults = serde_json::json!({
            "video": { "videoCodec": "h265", "bitrate": 12 },
            "advanced": { "customArgs": "" }
        });
        cfg.overrides.insert(
            id.clone(),
            OverrideEntry {
                enabled: true,
                values: serde_json::json!({ "video": { "maxFps": 30 } }),
            },
        );
        save_config_to(&path, &cfg).unwrap();

        let reloaded = load_config_from(&path).unwrap();

        assert_eq!(reloaded.defaults["video"]["bitrate"], 12);
        assert_eq!(reloaded.defaults["advanced"]["customArgs"], "");
        let entry = reloaded.overrides.get(&id).expect("覆盖应随配置一起保留");
        assert!(entry.enabled);
        assert_eq!(entry.values["video"]["maxFps"], 30);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn disabled_override_keeps_its_values() {
        // design D5：禁用只是不参与合并，值必须留在盘上（重启后仍能恢复）。
        let dir = unique_temp_dir("disabled");
        let path = config_path_in(&dir);

        let mut cfg = load_config_from(&path).unwrap();
        let id = cfg.upsert_device(usb("", "A", "S1")).unwrap();
        cfg.overrides.insert(
            id.clone(),
            OverrideEntry {
                enabled: false,
                values: serde_json::json!({ "window": { "fullscreen": true } }),
            },
        );
        save_config_to(&path, &cfg).unwrap();

        let reloaded = load_config_from(&path).unwrap();
        let entry = reloaded.overrides.get(&id).unwrap();

        assert!(!entry.enabled);
        assert_eq!(entry.values["window"]["fullscreen"], true);

        let _ = fs::remove_dir_all(&dir);
    }
}
