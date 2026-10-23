//! 无线调试的配对、连接端口解析与局域网发现
//! （spec `device-management` 的「无线调试的配对」「局域网无线设备的发现」「无线设备连接端口的解析」）。
//!
//! ## 为什么用 adb 服务端的 mDNS，而不是自己发组播
//! adb 自带 mDNS 客户端，host 服务 `host:mdns:check` / `host:mdns:services` 就能取到它已经
//! 发现的服务表。本模块只在这条既有连接上多发一个请求（[`crate::device::query_host`]），
//! 因此：**不引入 mDNS 依赖、不创建子进程、与 adb 自动连接看到的是同一份发现结果**。
//! 自己发组播会得到第二份视图 —— 同一台设备在两边不同时出现时，排查成本远高于省下的那点代码。
//!
//! ## 为什么端口解析是独立一步
//! Android 11 及以上配对成功后 adbd 监听一个**随机端口**，与 legacy `tcpip 5555` 不是同一个
//! socket；设备重启或重开无线调试后端口还会变。故 `5555` 只是「旧法设备的经验值」，
//! 不能当作缺省值去连接。解析出来的端口才是连接标识的端口部分。
//!
//! ## 扫描一律由用户触发
//! 设备状态的常规来源仍是 `host:track-devices` 订阅（spec `device-management` 的
//! 「设备状态由 adb 服务端订阅驱动」）。这里的扫描是**一次性动作**：没有定时器、没有后台线程，
//! 只有命令被调用的那几秒里才有网络活动。

use crate::assets::{self, ADB_BIN};
use crate::config::{self, Device};
use crate::device;
use crate::download::run_blocking;
use crate::logger;
use crate::scrcpy;
use crate::ui_message::{codes, UserError};
use crate::AppState;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream, UdpSocket};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};

/// 一次发现窗的时长：服务端要时间把已广播的服务收进表里，又不能久到像卡死。
pub const MDNS_WINDOW: Duration = Duration::from_secs(6);
/// 等待「待配对设备」出现的窗口：设备的配对弹窗本身是限时的，这里只需覆盖它已打开的情形。
pub const PAIR_WINDOW: Duration = Duration::from_secs(4);
/// 配对后等待连接服务出现的窗口。
pub const RESOLVE_WINDOW: Duration = Duration::from_secs(8);
/// 窗口内的轮询间隔。
const MDNS_POLL: Duration = Duration::from_millis(300);
/// `adb pair` 的超时：对不可达地址会阻塞到 TCP 超时，与 `connect` 同理只能靠外部终止。
pub const PAIR_TIMEOUT: Duration = Duration::from_secs(10);
/// 网段扫描的单次 TCP 探测超时（同网段正常 RTT 个位数毫秒）。
pub const PROBE_TIMEOUT: Duration = Duration::from_millis(300);
/// 网段扫描的整体预算（254 个地址 / 32 并发 ≈ 8 批 × 300ms）。
pub const SWEEP_BUDGET: Duration = Duration::from_secs(3);
/// 网段扫描的并发数。
const SWEEP_WORKERS: usize = 32;
/// 旧式 `adb tcpip` 的端口：只用于**兜底扫描**与手填地址的缺省端口，不作连接端口假设。
pub const LEGACY_PORT: u16 = 5555;

/// 扫描的取消标志。
///
/// 界面在扫描期间禁用入口，同一时刻只会跑一次扫描，故模块级一个标志足够；
/// 放进 `AppState` 只会多一处需要维护的状态。
static SCAN_CANCELLED: AtomicBool = AtomicBool::new(false);

fn begin_scan() {
    SCAN_CANCELLED.store(false, Ordering::SeqCst);
}

fn cancel_requested() -> bool {
    SCAN_CANCELLED.load(Ordering::SeqCst)
}

// ── mDNS 服务表 ────────────────────────────────────────────────

/// mDNS 服务类型：三种服务对应三种用途，混在一起会让「该连哪个端口」无从判断。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum MdnsKind {
    /// `_adb-tls-pairing._tcp`：配对服务，只在设备「使用配对码配对设备」弹窗打开期间广播。
    Pairing,
    /// `_adb-tls-connect._tcp`：无线调试的连接服务（端口随机）。
    Connect,
    /// `_adb._tcp`：旧式 `adb tcpip` 设备的连接服务（通常 5555）。
    Legacy,
}

/// 服务表里的一行。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MdnsService {
    /// 服务实例名（形如 `adb-14141FDF600081-QXjCrW`），仅用于日志。
    pub name: String,
    pub kind: MdnsKind,
    /// 已解析的 IPv4 地址（IPv6 行在解析阶段被丢弃）。
    pub host: String,
    pub port: u16,
}

/// 解析 `host:mdns:services` 的净荷（纯函数）。
///
/// 输出形如（表头 + 每行「实例名 服务类型 地址:端口」，分隔符可能是空格或制表符）：
///
/// ```text
/// List of discovered mdns services
/// adb-14141FDF600081 _adb._tcp  192.168.86.38:5555
/// adb-14141FDF600081-QXjCrW _adb-tls-pairing._tcp 192.168.86.38:33861
/// adb-14141FDF600081-TnSdi9 _adb-tls-connect._tcp 192.168.86.38:33015
/// ```
///
/// 只收已知的三种服务类型：其它类型（如 `studio-g@…` 这类别的工具广播的服务）不该进候选。
/// IPv6 行与端口非法行一并跳过 —— 调用方拿到的每一条都是可直接 `adb connect` 的 IPv4 端点。
pub fn parse_mdns_services(payload: &str) -> Vec<MdnsService> {
    payload
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() || line.starts_with("List of") {
                return None;
            }
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() != 3 {
                return None;
            }
            let kind = kind_of(parts[1])?;
            let (host, port) = parts[2].rsplit_once(':')?;
            let port: u16 = port.parse().ok()?;
            // IPv6 在这里被滤掉：`fe80::1` 会拆成 host `fe80:` + port `1`，IPv4 解析必然失败。
            host.parse::<Ipv4Addr>().ok()?;
            Some(MdnsService {
                name: parts[0].to_string(),
                kind,
                host: host.to_string(),
                port,
            })
        })
        .collect()
}

/// 服务类型 → 用途；未知类型返回 `None`。
fn kind_of(service_type: &str) -> Option<MdnsKind> {
    match service_type.trim().trim_end_matches('.') {
        "_adb-tls-pairing._tcp" => Some(MdnsKind::Pairing),
        "_adb-tls-connect._tcp" => Some(MdnsKind::Connect),
        "_adb._tcp" => Some(MdnsKind::Legacy),
        _ => None,
    }
}

/// 向 adb 服务端要一次服务表。
fn mdns_query(app: &AppHandle) -> Result<Vec<MdnsService>, String> {
    let payload = device::query_host(app, "host:mdns:services")?;
    Ok(parse_mdns_services(&payload))
}

// ── 发现窗 ─────────────────────────────────────────────────────

/// 在时间窗口内反复查询服务表并累积结果。
///
/// 单次查询拿的是服务端**当前**的表，项目刚广播时可能还是空的，故要给它一段时间。
/// `done` 命中即提前结束（端口解析只等自己那一台，没必要陪跑满整个窗口）。
///
/// 清点式失败的处理：**只要有一次查询成功就算这次扫描有效**（空结果也是有效结果）；
/// 一次都没成功才把最后一次的错误上抛 —— 否则「服务端还没起来」会被误报成「网络不支持」。
fn collect_services(
    window: Duration,
    keep: impl Fn(&MdnsService) -> bool,
    mut query: impl FnMut() -> Result<Vec<MdnsService>, String>,
    done: impl Fn(&[MdnsService]) -> bool,
) -> Result<Vec<MdnsService>, String> {
    begin_scan();
    let deadline = Instant::now() + window;
    let mut acc: BTreeMap<(String, u16), MdnsService> = BTreeMap::new();
    let mut last_err: Option<String> = None;
    let mut ok_once = false;
    loop {
        match query() {
            Ok(list) => {
                ok_once = true;
                for service in list.into_iter().filter(|s| keep(s)) {
                    acc.entry((service.host.clone(), service.port))
                        .or_insert(service);
                }
            }
            Err(e) => last_err = Some(e),
        }
        let collected: Vec<MdnsService> = acc.values().cloned().collect();
        if done(&collected) || cancel_requested() || Instant::now() >= deadline {
            break;
        }
        let left = deadline.saturating_duration_since(Instant::now());
        std::thread::sleep(MDNS_POLL.min(left));
    }
    if !ok_once && !cancel_requested() {
        return Err(last_err.unwrap_or_else(|| "无法查询 adb 的 mDNS 服务列表".to_string()));
    }
    Ok(acc.into_values().collect())
}

/// 从服务表里挑出某台设备的连接端口（纯函数）。
///
/// `prefer` 是设备记录里现有的端口：同一 IP 出现多条连接服务时优先沿用它，避免
/// 「解析一次换一个端口」来回跳；没有命中才取最小端口（结果稳定、可复现）。
pub fn pick_connect_port(services: &[MdnsService], ip: &str, prefer: Option<u16>) -> Option<u16> {
    let mut ports: Vec<u16> = services
        .iter()
        .filter(|s| s.host == ip && s.kind != MdnsKind::Pairing)
        .map(|s| s.port)
        .collect();
    ports.sort_unstable();
    ports.dedup();
    match prefer {
        Some(p) if ports.contains(&p) => Some(p),
        _ => ports.first().copied(),
    }
}

/// 解析某台设备的连接端口；窗口内没等到就报错（原因里带上 IP，便于对照）。
fn resolve_port(app: &AppHandle, ip: &str, prefer: Option<u16>) -> Result<u16, String> {
    let services = collect_services(
        RESOLVE_WINDOW,
        |_| true,
        || mdns_query(app),
        |list| pick_connect_port(list, ip, prefer).is_some(),
    )?;
    pick_connect_port(&services, ip, prefer)
        .ok_or_else(|| format!("未能解析出 {ip} 的连接端口：设备可能已关闭无线调试，或网络收不到多播"))
}

// ── 前端可见的结果形状 ─────────────────────────────────────────

/// 扫描结果里的一台设备。
#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveredDevice {
    pub ip: String,
    pub port: u16,
    /// 连接标识（`ip:端口`），与 `config::Device::connection_id()` 同形
    pub connection_id: String,
    /// 该 IP 已登记在设备列表（按 IP 判定：同一台设备的端口会漂，标识不能当身份）
    pub registered: bool,
    /// 来源：`mdns`（多播发现）或 `sweep`（5555 网段扫描）
    pub source: String,
}

/// 一次发现窗的结果。
///
/// mDNS 本身不可用时仍然返回 `Ok`，把原因放在 `mdns_error` 里 —— 界面据此决定要不要
/// 露出 `5555` 兜底入口，而不是把整次扫描判成失败。
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct WirelessScan {
    pub devices: Vec<DiscoveredDevice>,
    pub mdns_error: Option<UserError>,
}

/// 一台等待配对的设备（配对服务只在设备弹窗打开期间存在，故每次都要现查）。
#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PairingTarget {
    pub ip: String,
    pub port: u16,
    pub connection_id: String,
}

/// 配对 / 加入成功后回给界面的结果。
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct WirelessJoin {
    pub device_id: String,
    pub alias: String,
    pub connection_id: String,
}

// ── 扫描命令 ───────────────────────────────────────────────────

#[tauri::command]
pub async fn scan_wireless(app: AppHandle) -> Result<WirelessScan, UserError> {
    run_blocking(move || scan_blocking(&app)).await
}

fn scan_blocking(app: &AppHandle) -> Result<WirelessScan, UserError> {
    logger::info(
        "device",
        format!("开始扫描局域网无线设备（窗口 {} 秒）", MDNS_WINDOW.as_secs()),
    );
    let result = collect_services(
        MDNS_WINDOW,
        |s| s.kind != MdnsKind::Pairing,
        || mdns_query(app),
        |_| false,
    );
    match result {
        Ok(list) => {
            let devices = to_discovered(app, list, "mdns");
            logger::info("device", format!("扫描结束：发现 {} 台设备", devices.len()));
            Ok(WirelessScan {
                devices,
                mdns_error: None,
            })
        }
        Err(e) => {
            logger::warn("device", format!("多播发现不可用：{e}"));
            Ok(WirelessScan {
                devices: Vec::new(),
                mdns_error: Some(UserError::new(codes::WIRELESS_MDNS_FAILED).with_detail(e)),
            })
        }
    }
}

/// 把服务表收敛成去重后的结果，并标注「已登记」。
fn to_discovered(app: &AppHandle, services: Vec<MdnsService>, source: &str) -> Vec<DiscoveredDevice> {
    let cfg = app.state::<AppState>().config.lock().unwrap().clone();
    let mut seen: BTreeMap<(String, u16), DiscoveredDevice> = BTreeMap::new();
    for service in services {
        seen.entry((service.host.clone(), service.port))
            .or_insert_with(|| DiscoveredDevice {
                connection_id: format!("{}:{}", service.host, service.port),
                registered: cfg.devices.iter().any(|d| {
                    d.kind == config::KIND_WIFI && d.ip.trim() == service.host
                }),
                ip: service.host,
                port: service.port,
                source: source.to_string(),
            });
    }
    seen.into_values().collect()
}

#[tauri::command]
pub async fn scan_pairing_targets(app: AppHandle) -> Result<Vec<PairingTarget>, UserError> {
    run_blocking(move || {
        let services = collect_services(PAIR_WINDOW, |s| s.kind == MdnsKind::Pairing, || {
            mdns_query(&app)
        }, |_| false)
        .map_err(|e| UserError::new(codes::WIRELESS_MDNS_FAILED).with_detail(e))?;
        Ok(services
            .into_iter()
            .map(|s| PairingTarget {
                connection_id: format!("{}:{}", s.host, s.port),
                ip: s.host,
                port: s.port,
            })
            .collect())
    })
    .await
}

/// 取消进行中的扫描（发现窗与网段扫描都会在下一拍退出）。
#[tauri::command]
pub fn cancel_wireless_scan() {
    SCAN_CANCELLED.store(true, Ordering::SeqCst);
}

#[tauri::command]
pub async fn scan_wireless_ports(app: AppHandle) -> Result<Vec<DiscoveredDevice>, UserError> {
    run_blocking(move || sweep_blocking(&app)).await
}

fn sweep_blocking(app: &AppHandle) -> Result<Vec<DiscoveredDevice>, UserError> {
    let Some(local) = local_ipv4() else {
        return Err(UserError::new(codes::WIRELESS_SWEEP_UNAVAILABLE));
    };
    let hosts = subnet_hosts(local);
    logger::info(
        "device",
        format!(
            "本机出口 IPv4 为 {local}，开始扫描 {} 个地址的 {LEGACY_PORT} 端口",
            hosts.len()
        ),
    );
    begin_scan();
    let hits = sweep_ports(&hosts, LEGACY_PORT);
    if hits.is_empty() {
        logger::info("device", "端口扫描结束：没有探测到可连的端口");
        return Ok(Vec::new());
    }
    let dir = adb_dir(app)?;
    // 只对**探测通过**的候选调 `adb connect`：那条命令会阻塞 adb 服务端，
    // 对不可达地址盲调会把设备状态推送一起拖住。
    for host in &hits {
        if cancel_requested() {
            break;
        }
        let connection_id = format!("{host}:{LEGACY_PORT}");
        match device::connect_link(&dir, &connection_id) {
            Ok(_) => logger::info("device", format!("端口扫描命中并已发起连接：{connection_id}")),
            Err(e) => logger::warn("device", format!("端口扫描命中但连接失败（{connection_id}）：{e}")),
        }
    }
    device::refresh_probe(app);
    device::publish_status(app);
    // 成败以重探测为准：TCP 端口开着不等于这台机器上是可用的 adb 设备。
    let live = device::live_devices(app).unwrap_or_default();
    let services: Vec<MdnsService> = hits
        .into_iter()
        .filter(|host| live.contains(&format!("{host}:{LEGACY_PORT}")))
        .map(|host| MdnsService {
            name: host.to_string(),
            kind: MdnsKind::Legacy,
            host: host.to_string(),
            port: LEGACY_PORT,
        })
        .collect();
    let devices = to_discovered(app, services, "sweep");
    logger::info("device", format!("端口扫描结束：{} 台可连接", devices.len()));
    Ok(devices)
}

// ── 网段扫描（纯函数 + 一个阻塞实现） ───────────────────────────

/// 本机出口 IPv4 地址。
///
/// 用 UDP `connect` 查路由表决定的出口地址：UDP 的 connect **不发包**，离线环境同样可用，
/// 比枚举网卡少一整层平台 API（`GetAdaptersAddresses` 等）。
pub fn local_ipv4() -> Option<Ipv4Addr> {
    let sock = UdpSocket::bind("0.0.0.0:0").ok()?;
    sock.connect("8.8.8.8:80").ok()?;
    match sock.local_addr().ok()?.ip() {
        IpAddr::V4(v4) if !v4.is_loopback() && !v4.is_unspecified() => Some(v4),
        _ => None,
    }
}

/// 同一 `/24` 内除自己以外的其余地址（纯函数）。
pub fn subnet_hosts(ip: Ipv4Addr) -> Vec<Ipv4Addr> {
    let o = ip.octets();
    (1u8..=254)
        .map(|last| Ipv4Addr::new(o[0], o[1], o[2], last))
        .filter(|candidate| *candidate != ip)
        .collect()
}

/// 并发探一遍给定地址的 `5555` 端口，返回开着的那一批（升序）。
fn sweep_ports(hosts: &[Ipv4Addr], port: u16) -> Vec<Ipv4Addr> {
    use std::sync::atomic::AtomicUsize;
    use std::sync::Mutex;

    let cursor = AtomicUsize::new(0);
    let found: Mutex<Vec<Ipv4Addr>> = Mutex::new(Vec::new());
    let deadline = Instant::now() + SWEEP_BUDGET;
    let workers = SWEEP_WORKERS.min(hosts.len().max(1));
    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| loop {
                let index = cursor.fetch_add(1, Ordering::SeqCst);
                if index >= hosts.len() || cancel_requested() || Instant::now() >= deadline {
                    break;
                }
                if probe_port(hosts[index], port, PROBE_TIMEOUT) {
                    found.lock().unwrap().push(hosts[index]);
                }
            });
        }
    });
    let mut hits = found.into_inner().unwrap();
    hits.sort_unstable();
    hits
}

fn probe_port(host: Ipv4Addr, port: u16, timeout: Duration) -> bool {
    TcpStream::connect_timeout(&SocketAddr::new(IpAddr::V4(host), port), timeout).is_ok()
}

// ── 配对 ───────────────────────────────────────────────────────

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PairRequest {
    /// 设备的配对地址（`IP:配对端口`）；端口缺省按 5555 处理
    pub address: String,
    /// 设备上显示的 6 位配对码
    pub code: String,
    /// 用户指定的别名（留空则由机型推导）
    pub alias: Option<String>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct JoinRequest {
    pub ip: String,
    pub port: u16,
    pub alias: Option<String>,
}

/// `adb pair` 的成败判定（纯函数）。
///
/// **不看退出码**：与 `adb connect` 同类，失败时也可能以 0 退出。判定只认输出里的成功标识，
/// 失败则把原文带回 —— 用户要能区分「配对码错了」与「连不上那台设备」。
pub fn pair_outcome(output: &str) -> Result<(), String> {
    let text = output.trim();
    if text.to_ascii_lowercase().contains("successfully paired") {
        return Ok(());
    }
    Err(if text.is_empty() {
        "配对失败（adb 未给出原因）".to_string()
    } else {
        text.to_string()
    })
}

fn parse_address(raw: &str) -> Option<(String, u16)> {
    let text = raw.trim();
    if text.is_empty() {
        return None;
    }
    match text.rsplit_once(':') {
        Some((host, port)) => {
            let host = host.trim().trim_start_matches('[').trim_end_matches(']');
            let port: u16 = port.trim().parse().ok()?;
            if host.is_empty() || port == 0 {
                return None;
            }
            Some((host.to_string(), port))
        }
        None => Some((text.to_string(), LEGACY_PORT)),
    }
}

#[tauri::command]
pub async fn pair_wireless(app: AppHandle, request: PairRequest) -> Result<WirelessJoin, UserError> {
    run_blocking(move || pair_blocking(&app, request)).await
}

fn pair_blocking(app: &AppHandle, request: PairRequest) -> Result<WirelessJoin, UserError> {
    let dir = adb_dir(app)?;
    let (host, port) =
        parse_address(&request.address).ok_or_else(|| UserError::new(codes::WIRELESS_ADDRESS_INVALID))?;
    let code = request.code.trim();
    if code.is_empty() {
        return Err(UserError::new(codes::WIRELESS_CODE_REQUIRED));
    }
    let address = format!("{host}:{port}");
    let output = device::run_captured(
        &dir.join(ADB_BIN),
        &["pair", &address, code],
        PAIR_TIMEOUT,
    )
    .map_err(|e| UserError::new(codes::WIRELESS_PAIR_FAILED).with_detail(e))?;
    pair_outcome(&output)
        .map_err(|e| UserError::new(codes::WIRELESS_PAIR_FAILED).with_detail(e))?;
    logger::info("device", format!("无线配对成功：{address}"));
    // 配对端口与连接端口不是同一个：按 IP 解析出**连接**端口再建链。
    let connect_port = resolve_port(app, &host, None)
        .map_err(|e| UserError::new(codes::WIRELESS_PORT_UNRESOLVED).with_detail(e))?;
    logger::info(
        "device",
        format!("配对设备的连接端口已解析：{host}:{connect_port}"),
    );
    join_blocking(app, &dir, &host, connect_port, request.alias)
}

#[tauri::command]
pub async fn join_wireless(app: AppHandle, request: JoinRequest) -> Result<WirelessJoin, UserError> {
    run_blocking(move || {
        let dir = adb_dir(&app)?;
        join_blocking(&app, &dir, request.ip.trim(), request.port, request.alias)
    })
    .await
}

/// 建立链路、取机型作别名、登记设备（三件事的顺序不可换：没连上就不该看一眼型号）。
fn join_blocking(
    app: &AppHandle,
    dir: &Path,
    host: &str,
    port: u16,
    alias: Option<String>,
) -> Result<WirelessJoin, UserError> {
    let connection_id = format!("{host}:{port}");
    let output = device::connect_link(dir, &connection_id).map_err(|e| {
        UserError::new(codes::WIRELESS_CONNECT_FAILED).with_detail(e)
    })?;
    device::refresh_probe(app);
    device::publish_status(app);
    // 与 `session::link_outcome` 同一口径：成败看重探测结果，不看 `adb connect` 的退出码。
    let live = device::live_devices(app).unwrap_or_default();
    if !live.contains(&connection_id) {
        let detail = output.trim();
        return Err(UserError::new(codes::WIRELESS_CONNECT_FAILED).with_detail(
            if detail.is_empty() {
                "设备仍不可达".to_string()
            } else {
                detail.to_string()
            },
        ));
    }
    let model = device_model(dir, &connection_id);
    let alias = pick_alias(alias.as_deref(), model.as_deref(), host);
    let (device_id, alias) = register(app, host, port, &alias)?;
    Ok(WirelessJoin {
        device_id,
        alias,
        connection_id,
    })
}

/// 登记一台无线设备，返回（设备 id，最终别名）。
///
/// 事务语义与「删除设备要连带清覆盖」同源：落盘失败时内存与磁盘都不动。
fn register(
    app: &AppHandle,
    host: &str,
    port: u16,
    alias: &str,
) -> Result<(String, String), UserError> {
    let state = app.state::<AppState>();
    let mut created = false;
    let cfg = device::mutate_and_persist(app, &state, |cfg| {
        let (_, _, is_new) = upsert_wireless_record(cfg, host, port, alias)?;
        created = is_new;
        Ok(())
    })?;
    let record = cfg
        .devices
        .iter()
        .find(|d| d.kind == config::KIND_WIFI && d.ip.trim() == host)
        .ok_or_else(|| UserError::new(codes::DEVICE_INVALID))?;
    let out_id = record.id.clone();
    let out_alias = record.alias.clone();
    if created {
        logger::info(
            "device",
            format!("无线设备已登记：「{out_alias}」（连接标识 {host}:{port}，id {out_id}）"),
        );
    } else {
        logger::info(
            "device",
            format!("无线设备端口已更新：「{out_alias}」→ {host}:{port}"),
        );
    }
    device::publish_status(app);
    Ok((out_id, out_alias))
}

/// 把一条无线记录写进配置（纯函数，便于验证「同一 IP 只更新端口」与新增路径）。
///
/// **按 IP 识别既有记录**：同一台设备的端口会因设备侧重启而变，此时只更新端口，
/// id、别名与参数覆盖一律保留 —— 否则每重连一次就会多出一条记录、丢掉用户起的名字。
/// 返回（设备 id，最终别名，是否新增）。
pub fn upsert_wireless_record(
    cfg: &mut config::AppConfig,
    host: &str,
    port: u16,
    alias: &str,
) -> Result<(String, String, bool), UserError> {
    if let Some(existing) = cfg
        .devices
        .iter_mut()
        .find(|d| d.kind == config::KIND_WIFI && d.ip.trim() == host)
    {
        existing.port = port;
        return Ok((existing.id.clone(), existing.alias.clone(), false));
    }
    let device = Device {
        id: String::new(),
        alias: alias.to_string(),
        kind: config::KIND_WIFI.to_string(),
        serial: String::new(),
        ip: host.to_string(),
        port,
    };
    let id = cfg
        .upsert_device(device)
        .map_err(|e| UserError::new(codes::DEVICE_INVALID).with_detail(e))?;
    Ok((id, alias.to_string(), true))
}

/// 最终别名（纯函数）：用户显式指定 > 机型 > IP。
///
/// 机型是连接之后才取得到的，而别名要立刻落盘 —— 所以「取不到就退回 IP」必须是一条
/// 明确的规则，不能停在空字符串上（设备列表里一行没有名字的记录无法辨认）。
pub fn pick_alias(explicit: Option<&str>, model: Option<&str>, host: &str) -> String {
    explicit
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .or_else(|| {
            model
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
        })
        .unwrap_or_else(|| host.to_string())
}

/// 取设备的机型名作默认别名（取不到返回 `None`，由调用方退回 IP）。
///
/// 只取一个属性、在设备侧过滤，输出很小 —— 满足 `run_captured`「只用于小输出」的前提。
fn device_model(dir: &Path, connection_id: &str) -> Option<String> {
    let out = device::run_captured(
        &dir.join(ADB_BIN),
        &["-s", connection_id, "shell", "getprop ro.product.model"],
        device::PROBE_TIMEOUT,
    )
    .ok()?;
    let name = out.trim();
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

/// 解析运行环境目录并要求必需的可执行文件齐备（与 `session::start_session` 同一口径）。
fn adb_dir(app: &AppHandle) -> Result<PathBuf, UserError> {
    let dir = scrcpy::scrcpy_dir(app)
        .map_err(|e| UserError::new(codes::SESSION_ENVIRONMENT_NOT_READY).with_detail(e))?;
    let missing = assets::missing_bins(&dir);
    if !missing.is_empty() {
        return Err(UserError::new(codes::SESSION_ENVIRONMENT_NOT_READY)
            .with_detail(missing.join("、")));
    }
    Ok(dir)
}

#[tauri::command]
pub async fn resolve_wifi_port(
    app: AppHandle,
    device_id: String,
) -> Result<WirelessJoin, UserError> {
    run_blocking(move || resolve_blocking(&app, &device_id)).await
}

fn resolve_blocking(app: &AppHandle, device_id: &str) -> Result<WirelessJoin, UserError> {
    let (ip, current) = {
        let state = app.state::<AppState>();
        let cfg = state.config.lock().unwrap();
        let device = cfg
            .devices
            .iter()
            .find(|d| d.id == device_id)
            .ok_or_else(|| UserError::new(codes::DEVICE_NOT_FOUND))?;
        if device.kind != config::KIND_WIFI {
            return Err(UserError::new(codes::WIRELESS_NOT_WIRELESS));
        }
        (device.ip.trim().to_string(), device.port)
    };
    let port = resolve_port(app, &ip, Some(current)).map_err(|e| {
        UserError::new(codes::WIRELESS_PORT_UNRESOLVED).with_detail(e)
    })?;
    logger::info(
        "device",
        format!("已重新解析「{ip}」的连接端口：{current} → {port}"),
    );
    let dir = adb_dir(app)?;
    // 端口只在这条链路真的建立之后才写回记录（`join_blocking` 里完成）——
    // 既有规则「链路建立失败不得改动设备记录」优先于「解析成功即写回」。
    join_blocking(app, &dir, &ip, port, None)
}

#[cfg(test)]
mod tests {
    //! 可验证部分：服务表解析、端口挑选、地址解析、配对判定、网段展开全是纯函数；
    //! 网络与进程部分不在此处验证（扫描窗口与 TCP 探测依赖真实环境）。

    use super::*;

    #[test]
    fn parses_the_real_service_table() {
        // 实测输出：表头 + 「实例名 类型 地址:端口」，分隔符可能是空格或制表符。
        let payload = "List of discovered mdns services\n\
                       adb-14141FDF600081 _adb._tcp\t 192.168.86.38:5555\n\
                       adb-14141FDF600081-QXjCrW _adb-tls-pairing._tcp 192.168.86.38:33861\n\
                       adb-14141FDF600081-TnSdi9 _adb-tls-connect._tcp 192.168.86.38:33015\n";

        let got = parse_mdns_services(payload);

        assert_eq!(got.len(), 3);
        assert_eq!(got[0].kind, MdnsKind::Legacy);
        assert_eq!(got[1].kind, MdnsKind::Pairing);
        assert_eq!(got[2].kind, MdnsKind::Connect);
        assert_eq!(got[1].host, "192.168.86.38");
        assert_eq!(got[1].port, 33861);
        assert_eq!(got[0].name, "adb-14141FDF600081");
    }

    #[test]
    fn empty_table_is_not_an_error() {
        // 实测：没有设备时只有表头一行。
        assert!(parse_mdns_services("List of discovered mdns services\n").is_empty());
        assert!(parse_mdns_services("").is_empty());
    }

    #[test]
    fn skips_unknown_types_ipv6_and_bad_ports() {
        let payload = "List of discovered mdns services\n\
                       studio-g@abc _adb-tls-pairing._tcp 192.168.86.40:1\n\
                       someone-else _http._tcp 192.168.86.41:80\n\
                       adb-v6 _adb-tls-connect._tcp fe80::1:33015\n\
                       adb-bad _adb-tls-connect._tcp 192.168.86.42:not-a-port\n\
                       adb-v4 _adb-tls-connect._tcp 192.168.86.43:33015\n";

        let got = parse_mdns_services(payload);

        // 别的工具广播的 `_adb-tls-pairing` 属于已知类型，保留（配对时由用户选择）；
        // `_http._tcp`、IPv6、非法端口一律丢弃。
        assert_eq!(got.len(), 2, "实际：{got:?}");
        assert!(got.iter().all(|s| s.kind == MdnsKind::Pairing && s.host == "192.168.86.40"
            || s.kind == MdnsKind::Connect && s.host == "192.168.86.43"));
    }

    #[test]
    fn picks_the_connect_port_and_never_the_pairing_one() {
        let services = vec![
            MdnsService {
                name: "pair".into(),
                kind: MdnsKind::Pairing,
                host: "192.168.1.5".into(),
                port: 33861,
            },
            MdnsService {
                name: "conn".into(),
                kind: MdnsKind::Connect,
                host: "192.168.1.5".into(),
                port: 33015,
            },
        ];

        // 配对端口绝不能被当成连接端口。
        assert_eq!(pick_connect_port(&services, "192.168.1.5", None), Some(33015));
        // 另一台设备不命中。
        assert_eq!(pick_connect_port(&services, "192.168.1.6", None), None);
    }

    #[test]
    fn prefers_the_stored_port_when_it_still_exists() {
        let services = vec![
            MdnsService {
                name: "a".into(),
                kind: MdnsKind::Connect,
                host: "192.168.1.5".into(),
                port: 40001,
            },
            MdnsService {
                name: "b".into(),
                kind: MdnsKind::Connect,
                host: "192.168.1.5".into(),
                port: 40002,
            },
        ];

        // 既有端口仍在 → 沿用，避免解析一次换一个；已消失 → 取最小（结果稳定）。
        assert_eq!(pick_connect_port(&services, "192.168.1.5", Some(40002)), Some(40002));
        assert_eq!(pick_connect_port(&services, "192.168.1.5", Some(39999)), Some(40001));
    }

    #[test]
    fn parses_pairing_addresses() {
        assert_eq!(
            parse_address("192.168.1.5:37123"),
            Some(("192.168.1.5".to_string(), 37123))
        );
        // 只给 IP：端口按旧式缺省补齐（用户手填的兜底路径）。
        assert_eq!(
            parse_address(" 192.168.1.5 "),
            Some(("192.168.1.5".to_string(), LEGACY_PORT))
        );
        assert_eq!(parse_address(""), None);
        assert_eq!(parse_address("192.168.1.5:0"), None);
        assert_eq!(parse_address(":5555"), None);
    }

    #[test]
    fn pair_outcome_reads_the_output_not_the_exit_code() {
        assert!(pair_outcome("Successfully paired to 192.168.1.5:37123 [guid=adb-ABC]").is_ok());
        // 大小写不敏感：不同 adb 版本的写法不完全一致。
        assert!(pair_outcome("successfully paired").is_ok());

        let err = pair_outcome("Failed: Wrong password").unwrap_err();
        assert!(err.contains("Wrong password"), "必须保留原文：{err}");

        // 空输出给一句可读的兜底，而不是空原因。
        assert!(pair_outcome("   ").unwrap_err().contains("未给出原因"));
    }

    #[test]
    fn subnet_covers_the_whole_slash_24_but_not_self() {
        let hosts = subnet_hosts(Ipv4Addr::new(192, 168, 1, 20));

        assert_eq!(hosts.len(), 253);
        assert!(hosts.contains(&Ipv4Addr::new(192, 168, 1, 1)));
        assert!(hosts.contains(&Ipv4Addr::new(192, 168, 1, 254)));
        assert!(!hosts.contains(&Ipv4Addr::new(192, 168, 1, 20)), "不该探自己");
        // 不跨 /24：别的网段一个都不该出现。
        assert!(!hosts.iter().any(|ip| !ip.to_string().starts_with("192.168.1.")));
    }

    // ── 发现窗 ─────────────────────────────────────────────────

    fn svc(kind: MdnsKind, host: &str, port: u16) -> MdnsService {
        MdnsService {
            name: format!("{host}-{port}"),
            kind,
            host: host.to_string(),
            port,
        }
    }

    #[test]
    fn collect_window_dedupes_across_polls() {
        // 同一台设备在多次查询里都出现（每次都是完整快照）：结果里只应有一条。
        let polls = std::cell::Cell::new(0);
        let got = collect_services(
            Duration::from_millis(400),
            |_| true,
            || {
                polls.set(polls.get() + 1);
                Ok(vec![
                    svc(MdnsKind::Connect, "192.168.1.20", 40301),
                    svc(MdnsKind::Legacy, "192.168.1.31", 5555),
                ])
            },
            |_| false,
        )
        .unwrap();

        assert_eq!(got.len(), 2);
        assert!(polls.get() > 1, "窗口内应轮询多次（实际 {} 次）", polls.get());
    }

    #[test]
    fn collect_window_stops_early_when_done() {
        // 端口解析只等自己那一台：`done` 一命中就走，不陪跑满整个窗口。
        let polls = std::cell::Cell::new(0);
        let started = Instant::now();
        let got = collect_services(
            Duration::from_secs(8),
            |_| true,
            || {
                polls.set(polls.get() + 1);
                Ok(vec![svc(MdnsKind::Connect, "192.168.1.20", 40301)])
            },
            |list| !list.is_empty(),
        )
        .unwrap();

        assert_eq!(got.len(), 1);
        assert_eq!(polls.get(), 1, "命中后不得再查");
        assert!(started.elapsed() < Duration::from_secs(2), "应立刻返回");
    }

    #[test]
    fn collect_window_reports_only_when_every_poll_failed() {
        // 先失败后成功：算这次扫描有效（服务端可能刚起来），不能报错。
        let polls = std::cell::Cell::new(0);
        let got = collect_services(
            Duration::from_millis(350),
            |_| true,
            || {
                let n = polls.get();
                polls.set(n + 1);
                if n == 0 {
                    Err("adb 服务端还没起来".to_string())
                } else {
                    Ok(vec![svc(MdnsKind::Connect, "192.168.1.20", 40301)])
                }
            },
            |list| !list.is_empty(),
        )
        .unwrap();
        assert_eq!(got.len(), 1);

        // 一次都没成功：把最后一次的错误上抛 —— 否则「服务端没起来」会被误报成「网络不支持」。
        let err = collect_services(
            Duration::from_millis(120),
            |_| true,
            || Err("unknown host service".to_string()),
            |_| false,
        )
        .unwrap_err();
        assert!(err.contains("unknown host service"), "实际：{err}");
    }

    #[test]
    fn sweep_finds_only_the_open_port() {
        // 开一个本地监听当作「有 adb 的机器」，另一个刚释放的端口当作「没有」。
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let open = listener.local_addr().unwrap().port();
        let closed = {
            let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            l.local_addr().unwrap().port()
        };

        assert!(probe_port(Ipv4Addr::LOCALHOST, open, PROBE_TIMEOUT));
        assert!(!probe_port(Ipv4Addr::LOCALHOST, closed, PROBE_TIMEOUT));
        drop(listener);
    }

    #[test]
    fn local_address_lookup_never_panics() {
        // 取不到出口地址是允许的结果（无路由的机器），但不得 panic。
        if let Some(ip) = local_ipv4() {
            assert!(!ip.is_loopback());
        }
    }

    // ── 登记与别名 ─────────────────────────────────────────────

    fn wifi_record(id: &str, alias: &str, ip: &str, port: u16) -> Device {
        Device {
            id: id.to_string(),
            alias: alias.to_string(),
            kind: config::KIND_WIFI.to_string(),
            serial: String::new(),
            ip: ip.to_string(),
            port,
        }
    }

    #[test]
    fn register_updates_the_port_of_the_same_ip_and_keeps_identity() {
        let mut cfg = config::AppConfig::fresh();
        let id = cfg
            .upsert_device(wifi_record("", "我的手机", "192.168.1.20", 40301))
            .unwrap();

        // 同一台设备重开无线调试、端口变了：只更新端口，id 与别名都不动。
        let (got_id, got_alias, created) =
            upsert_wireless_record(&mut cfg, "192.168.1.20", 41234, "不该被用上").unwrap();

        assert_eq!(got_id, id, "端口变化不应换 id");
        assert_eq!(got_alias, "我的手机", "用户起的名字不能被覆盖");
        assert!(!created, "这是更新，不是新增");
        assert_eq!(cfg.devices.len(), 1, "不应多出一条记录");
        assert_eq!(cfg.devices[0].port, 41234);
        assert_eq!(cfg.devices[0].connection_id(), "192.168.1.20:41234");
    }

    #[test]
    fn register_creates_a_record_for_an_unknown_ip() {
        let mut cfg = config::AppConfig::fresh();
        // 已有另一台无线设备：新 IP 必须新增，且不动既有那条。
        cfg.upsert_device(wifi_record("", "平板", "192.168.1.31", 5555))
            .unwrap();

        let (id, alias, created) =
            upsert_wireless_record(&mut cfg, "192.168.1.20", 40301, "Redmi K60").unwrap();

        assert!(created);
        assert_eq!(alias, "Redmi K60");
        assert!(!id.is_empty(), "id 由后端分配");
        assert_eq!(cfg.devices.len(), 2);
        assert_eq!(cfg.devices[1].connection_id(), "192.168.1.20:40301");
    }

    #[test]
    fn registering_the_same_address_twice_does_not_duplicate() {
        let mut cfg = config::AppConfig::fresh();
        upsert_wireless_record(&mut cfg, "192.168.1.20", 40301, "手机").unwrap();
        let (_, _, created) = upsert_wireless_record(&mut cfg, "192.168.1.20", 40301, "手机").unwrap();

        assert!(!created);
        assert_eq!(cfg.devices.len(), 1, "同一 IP 重复登记不得新增记录");
    }

    #[test]
    fn registered_port_survives_a_restart() {
        // 走真实落盘路径（与 `register` 同一对函数）：重启后端口仍是解析出来的那个，
        // 否则用户每次重开应用都要重新解析一遍。
        use std::time::{SystemTime, UNIX_EPOCH};
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "fcn-wireless-{}-{}",
            std::process::id(),
            nanos
        ));
        let path = config::config_path_in(&dir);

        let mut cfg = config::load_config_from(&path).unwrap();
        upsert_wireless_record(&mut cfg, "192.168.1.20", 40301, "手机").unwrap();
        // 再更新一次端口：盘上应是最后那个值，且没有多出第二条记录。
        upsert_wireless_record(&mut cfg, "192.168.1.20", 41234, "手机").unwrap();
        config::save_config_to(&path, &cfg).unwrap();

        let reloaded = config::load_config_from(&path).unwrap();
        assert_eq!(reloaded.devices.len(), 1, "同一 IP 不得留下两条记录");
        assert_eq!(reloaded.devices[0].connection_id(), "192.168.1.20:41234");
        assert_eq!(reloaded.devices[0].alias, "手机");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn alias_falls_back_from_explicit_to_model_to_ip() {
        // 用户显式指定优先（含前后空白归一）。
        assert_eq!(pick_alias(Some("  书桌机 "), Some("Pixel 8"), "10.0.0.2"), "书桌机");
        // 没指定 → 机型。
        assert_eq!(pick_alias(None, Some("Pixel 8"), "10.0.0.2"), "Pixel 8");
        // 机型取不到（老系统 / getprop 失败）→ IP，绝不留空。
        assert_eq!(pick_alias(None, None, "10.0.0.2"), "10.0.0.2");
        // 空串与纯空白一律当作「没指定」。
        assert_eq!(pick_alias(Some("   "), None, "10.0.0.2"), "10.0.0.2");
        assert_eq!(pick_alias(None, Some("  "), "10.0.0.2"), "10.0.0.2");
    }
}
