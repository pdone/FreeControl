//! 运行环境获取：下载通道 · 官方校验 · 原子替换（tasks 4.1–4.9 / design D3 · D4）。
//!
//! 本模块是 `assets` 运行环境职责中「获取」那一半：`assets` 负责「目录在哪、是否就绪」，
//! 本模块负责「把二进制弄进来、且不弄坏现有环境」。
//!
//! ## 下载通道（design D9）
//! 通道按序为：①GitHub 官方原始链接（默认）②配置文件里的代理地址（出厂默认 + 用户追加）。
//! 代理地址只有一处来源：`config.channels.proxies` —— 本模块**不声明任何地址**，
//! 因此也不存在「内置 / 自定义」的二分：列表内一律可删，界面上的只读行只有 GitHub 直连。
//! 代理拼接形式统一为 `{proxy}/{原始 GitHub URL}`（见 `Channel::url_for`）。
//! 某一通道失败即自动降级到下一个；全部失败则终止并在错误信息里逐条列出失败原因。
//!
//! ## 人工干预（design D1 / D2 / D6）
//! 网络路径全部跑在 async `reqwest` 上：只有 async 侧才有 `read_timeout`（停滞预算），
//! 也只有它能用 `select!` 把「取消」与「切换通道」做得真正即时 —— 阻塞版的一次读
//! 无法从外部打断。解压与原子替换仍是阻塞工作，继续走阻塞线程池。
//! 「取消」不是失败：以 `status = "canceled"` 的结论返回，前端据此收起进度块而**不报错**。
//!
//! ## 完整性校验（design D4 / tasks 4.4–4.5）
//! 校验值来源为官方 `SHA256SUMS` 汇总文件，且**从本次实际生效的通道**获取：
//! 下载完成后按本次资产的文件名在其中定位条目（兼容 GNU coreutils 的 `*` 前缀写法），
//! 取出 SHA-256 与本地计算结果比对；不通过则拒绝解压与写入，原运行环境保持不变。
//! 若该文件拉取失败、或其中缺少本次资产的条目，则判定为「未经校验」，
//! 明确告知用户并要求确认后才继续（不得静默放行，也不得直接失败）。
//!
//! ## 落盘（tasks 4.6–4.7）
//! 「临时目录下载 → 校验 → 解压 → 校验必需文件 → 原子替换」。替换开始前先确认没有本应用
//! 会话在运行（有则要求先断开），再结束由设备状态探测拉起的 adb 服务端、确认目录可替换；
//! 任一步失败都清掉临时目录，且不影响原目录。
//!
//! ## 平台（design D1 / D3）
//! 资产名、运行环境目录名与解压方式都由**当前平台的标签与压缩格式**决定
//! （`scrcpy::current()`）：Windows 是 `scrcpy-win64-v{版本}.zip`，
//! Linux 是 `scrcpy-linux-x86_64-v{版本}.tar.gz`。平台不在受支持表内时，获取流程以明确
//! 错误结束，MUST NOT 退而取回别的平台的资产（那会「装上了但跑不起来」）。
//!
//! ## 下载目标版本（spec `scrcpy-environment`）
//! - 首次启动的自动获取：**最佳适配版本**（`scrcpy::BEST_FIT_VERSION`，确定性、不依赖 API）；
//! - 「下载最佳适配版本」：**构建期锁定的受支持版本**（`scrcpy::BEST_FIT_VERSION`，
//!   确定性、不依赖 API）；
//! - 「下载最新版本」：**官方最新发布版本**（经 GitHub Releases API 解析；查询失败时
//!   回退到最佳适配版本并在结果里说明）。两者都装入同一个约定目录（目录名按 best-fit 拼），
//!   因此后者可能装入与最佳适配不一致的版本 —— 由 `scrcpy::get_compat_status` 给出黄色叹号警示。

use crate::{assets, config, device, logger, scrcpy, session, AppState};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::cmp::Ordering;
use std::collections::HashSet;
use std::fs;
use std::future::Future;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU8, Ordering as AtomicOrdering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::Notify;

// ── 地址与常量的唯一来源 ───────────────────────────────────

/// 官方发布资产基址：`{base}/v{version}/{asset}`。
pub const RELEASES_BASE: &str = "https://github.com/Genymobile/scrcpy/releases/download";

/// GitHub Releases「最新发布」接口（仅用于「检查更新」与「下载最新版本」）。
pub const LATEST_RELEASE_API: &str =
    "https://api.github.com/repos/Genymobile/scrcpy/releases/latest";

/// 官方校验值汇总文件的**候选名**（按序尝试，取第一个可获取者）。
///
/// 核对 scrcpy v5.0.1 官方发布资产后确认：实际资产名为 `SHA256SUMS.txt`；
/// 而 design D4 的写法为 `SHA256SUMS`（无扩展名）。两者同源、内容格式一致
/// （`<sha256>  <文件名>`，本套以 `.txt` 为准），故按序尝试以免任一方写法变化时失效。
pub const SUMS_CANDIDATES: [&str; 2] = ["SHA256SUMS.txt", "SHA256SUMS"];

/// 校验值汇总文件的展示名（界面文案与错误信息引用它）。
pub const SUMS_ASSET: &str = SUMS_CANDIDATES[0];

/// GitHub 发布页：界面上「下载通道」列表的**首行只读项**（design D9）。
///
/// 与 [`RELEASES_BASE`] 是两个不同用途的地址：后者用于拼资产地址（`…/releases/download`），
/// 前者是给人看的发布页。两者不共用、也不互相推导。
///
/// 这一项**不可点击、也无跳转**：它是纯展示文本，与列表内的代理行同款。
/// （早先为它提供过一条「在系统浏览器中打开发布页」的命令，已随「只读项不做跳转」一起退役。）
pub const RELEASES_PAGE: &str = "https://github.com/Genymobile/scrcpy/releases/";

/// 临时目录前缀（位于应用数据根下，与目标目录**同盘**，保证 rename 为原子操作）。
const STAGING_PREFIX: &str = ".staging-scrcpy-";

/// 下载进度事件名（前端以此渲染进度条与当前通道）。
pub const PROGRESS_EVENT: &str = "scrcpy-install-progress";

// ── 资产名与通道 ───────────────────────────────────────────

/// 发布资产文件名：`scrcpy-{平台标签}-v{版本}.{压缩后缀}`（与官方发布资产、约定目录名同构）。
///
/// 平台标签与压缩后缀都取自 [`scrcpy::Platform`]，与运行环境目录名同源 —— 一处取值、两处引用
/// （design D3）。接参数而非直接用 `current()`：这样一条单测就能覆盖两行取值。
pub fn asset_at(platform: &scrcpy::Platform, version: &str) -> String {
    format!("scrcpy-{}-v{}.{}", platform.tag, version, platform.ext)
}

/// 把同一发布目录下的兄弟资产地址由 `asset_url` 推出（如 `SHA256SUMS`）。
pub fn sibling_url(asset_url: &str, name: &str) -> Option<String> {
    let idx = asset_url.rfind('/')?;
    Some(format!("{}/{}", &asset_url[..idx], name))
}

/// 下载通道。官方通道与代理通道的区别只体现在 `url_for` 的拼接上。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Channel {
    /// GitHub 官方原始链接（默认通道）
    Official,
    /// 代理通道：地址来自配置文件，列表内一律可删（design D9）
    Proxy { url: String },
}

impl Channel {
    /// 界面展示用标签（design D8「标明当前所用通道」）。
    pub fn label(&self) -> String {
        match self {
            Channel::Official => "GitHub 直连".to_string(),
            Channel::Proxy { url } => format!("代理 {}", url),
        }
    }

    /// 该通道下的真实请求地址：官方直接用原始链接，代理则前缀拼接（design D3）。
    pub fn url_for(&self, origin: &str) -> String {
        match self {
            Channel::Official => origin.to_string(),
            Channel::Proxy { url } => {
                format!("{}/{}", url.trim_end_matches('/'), origin.trim_start_matches('/'))
            }
        }
    }
}

/// 解析可用通道列表：GitHub 官方原始链接（默认）→ 配置文件里的代理地址（design D9）。
///
/// 代理项来自 `config.json`，可能被手工改坏，故此处过滤非法格式；
/// 出厂默认地址就在这份配置里、与用户追加项走同一条路径 —— 不存在特权项，
/// 因此也不存在「内置项不可删」这类需要在这里单独兜底的规则。
pub fn resolve_channels(proxies: &[String]) -> Vec<Channel> {
    let mut list = vec![Channel::Official];
    list.extend(proxies.iter().filter_map(|raw| {
        validate_proxy_address(raw)
            .ok()
            .map(|url| Channel::Proxy { url })
    }));
    list
}

/// 校验并规范化用户追加的代理地址（tasks 4.1：格式非法拒绝保存并提示）。
///
/// 只接受 `http://` / `https://` 前缀且带主机名（`https://` 后面直接跟路径或空串都不算合法），
/// 返回值去掉多余的尾部 `/`，便于统一拼接。
pub fn validate_proxy_address(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("地址不能为空".to_string());
    }
    let rest = if let Some(r) = trimmed.strip_prefix("https://") {
        r
    } else if let Some(r) = trimmed.strip_prefix("http://") {
        r
    } else {
        return Err("地址必须以 http:// 或 https:// 开头".to_string());
    };
    if rest.trim_matches('/').is_empty() {
        return Err("地址缺少主机名".to_string());
    }
    if trimmed.chars().any(|c| c.is_whitespace()) {
        return Err("地址中不能包含空格".to_string());
    }
    Ok(trimmed.trim_end_matches('/').to_string())
}

// ── 通道记忆（design D1 / D6） ─────────────────────────────

/// 本次应用运行内的通道负向记忆（design D1 / D4）。
///
/// 两类分开存：
/// - `unreachable`：**无法建立连接**（连接失败或连接阶段超时）；
/// - `manual`：用户点「切换通道」主动跳过的（语义是「太慢」，不是「连不上」）。
///
/// 不让「切换通道」直接写进 `unreachable` 的理由：那会让界面上展示的失败原因与后续
/// 实际跳过行为对不上，也让「仅由 HTTP 状态码判定的失败 MUST NOT 进入记忆」这类边界
/// 无从表述。两者的寿命相同：只存内存、**不落盘**（位置本身就保证了「每次启动仍以
/// 官方通道为第一个尝试对象」），一次成功安装完成后一并清空。
///
/// 键取 `Channel` 值本身（用其已有的 `PartialEq` 去重），不另立字符串键 ——
/// 免得「改一处标签文案就让记忆静默失效」成为新的隐性约定。
#[derive(Default)]
pub struct ChannelMemory {
    unreachable: Vec<Channel>,
    manual: Vec<Channel>,
}

impl ChannelMemory {
    /// 记下一条「连不上」的通道（去重）。
    pub fn mark_unreachable(&mut self, channel: &Channel) {
        push_unique(&mut self.unreachable, channel);
    }

    /// 记下一条被用户主动跳过的通道（去重）。
    pub fn mark_skipped(&mut self, channel: &Channel) {
        push_unique(&mut self.manual, channel);
    }

    /// 该通道是否已在本次运行内被判定为无法建立连接。
    pub fn skipped(&self, channel: &Channel) -> bool {
        self.unreachable.contains(channel)
    }

    /// 该通道是否被用户主动跳过（design D4）。
    pub fn manually_skipped(&self, channel: &Channel) -> bool {
        self.manual.contains(channel)
    }

    /// 一次下载成功完成后清空（design D7 / D4）：下一次获取重新从默认通道开始尝试。
    pub fn clear(&mut self) {
        self.unreachable.clear();
        self.manual.clear();
    }
}

fn push_unique(list: &mut Vec<Channel>, channel: &Channel) {
    if !list.contains(channel) {
        list.push(channel.clone());
    }
}

/// 本次真正要尝试的通道：跳过「无法建立连接」与「被用户主动跳过」的通道，其余保持原顺序
/// （design D6 / D4）。
///
/// 过滤后为空则**原样返回全部** —— 记忆不允许把一次获取变成「无通道可试」的假失败：
/// 那会让界面报出「全部下载通道均失败」，而实际上一条通道都没试过。
pub fn usable_channels(memory: &ChannelMemory, channels: &[Channel]) -> Vec<Channel> {
    let filtered: Vec<Channel> = channels
        .iter()
        .filter(|channel| !memory.skipped(channel) && !memory.manually_skipped(channel))
        .cloned()
        .collect();
    if filtered.is_empty() {
        channels.to_vec()
    } else {
        filtered
    }
}

// ── 按序降级 ───────────────────────────────────────────────

// ── 人工干预：切换通道 / 取消（design D2 / D4 / D6） ────────

/// 用户对在途获取下达的命令。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CtlCmd {
    /// 无命令（初始值，也是消费后的复位值）
    Run,
    /// 点了「切换通道」：中断当前这次尝试，改用下一条
    SkipChannel,
    /// 点了「取消」：终止本次获取
    Cancel,
}

const CMD_RUN: u8 = 0;
const CMD_SKIP: u8 = 1;
const CMD_CANCEL: u8 = 2;

/// 一次在途获取的人工控制入口（design D2）。
///
/// 为什么是 `Notify` + 原子命令字，而不是 `watch`：
/// 本结构的所有方法都只借 `&self`，于是「一次通道尝试」的 async 闭包只捕获**共享引用**，
/// 才能既被 `attempt_channels` 反复调用（`FnMut`），又在每次调用里等待控制信号。
/// `watch::Receiver::changed()` 要 `&mut self`，那会迫使该闭包可变借走捕获项，
/// 从而无法在 `FnMut` 里返回借用它的 future（编译器会拒绝：捕获项不能逃出闭包体）。
///
/// 命令字是「读取即复位」的：`take_if_any()` 读走命令并复位为 `Run`，
/// 因此同一次安装里连续两次「切换通道」不会被去重掉 —— `Notify` 的 permit 会合并，
/// 但合并掉的只是一次空唤醒，循环每轮都会先显式取一次命令字。
pub struct InstallCtl {
    cmd: AtomicU8,
    notify: Notify,
}

impl InstallCtl {
    pub fn new() -> Self {
        InstallCtl {
            cmd: AtomicU8::new(CMD_RUN),
            notify: Notify::new(),
        }
    }

    /// 请求切换到下一条通道（design D6）。
    pub fn request_skip(&self) {
        self.cmd.store(CMD_SKIP, AtomicOrdering::SeqCst);
        self.notify.notify_waiters();
    }

    /// 请求取消本次获取（design D6）。
    pub fn request_cancel(&self) {
        self.cmd.store(CMD_CANCEL, AtomicOrdering::SeqCst);
        self.notify.notify_waiters();
    }

    /// 取走当前命令并复位为 `Run`；无命令时返回 `None`。
    pub fn take_if_any(&self) -> Option<CtlCmd> {
        match self.cmd.swap(CMD_RUN, AtomicOrdering::SeqCst) {
            CMD_SKIP => Some(CtlCmd::SkipChannel),
            CMD_CANCEL => Some(CtlCmd::Cancel),
            _ => None,
        }
    }

    /// 等待一次控制信号；被唤醒后由调用方 `take_if_any()` 取值。
    pub async fn wait(&self) {
        self.notify.notified().await;
    }
}

impl Default for InstallCtl {
    fn default() -> Self {
        InstallCtl::new()
    }
}

/// 把命令翻译成「本次尝试为什么提前结束」（design D5）。
fn ctl_abort(cmd: CtlCmd) -> AttemptError {
    match cmd {
        CtlCmd::Cancel => AttemptError::Canceled,
        // `Run` 是复位值，走不到这里；当作「切换」处理最保守（只是再试下一条）。
        _ => AttemptError::Switched,
    }
}

/// 一次通道尝试的结束方式（design D5）。
///
/// 失败原因一旦被字符串化，「用户切换」「用户取消」与「通道失败」就不可区分 ——
/// 界面会把一条用户指令并进失败清单，用户看到的是一堆自己没遇到过的失败；
/// 而取消更不该出现在任何错误里。
#[derive(Debug)]
pub enum AttemptError {
    /// 该通道这次不行（可读原因 + 是否属于「无法建立连接」）
    Failure(AttemptFailure),
    /// 用户点了「切换通道」：本条不计失败，直接跳到下一条
    Switched,
    /// 用户点了「取消」：整个获取流程终止
    Canceled,
}

/// 一次获取流程的提前结束（design D5）。
#[derive(Debug)]
pub enum InstallAbort {
    /// 全部可用通道都没成；可读信息里逐条列出原因
    AllFailed(String),
    /// 用户取消（不是失败，见 design D6）
    Canceled,
}

/// 按通道顺序执行一次请求，返回首个成功的结果与生效通道（design D5）。
///
/// 每次尝试**之前**先回调 `on_attempt`（序号 / 总数 / 通道），使界面在首个数据块到达之前
/// 就能显示正在尝试哪条通道；失败是否写入通道记忆由 `AttemptFailure.unreachable` 决定；
/// `Switched` 表示用户主动跳过 —— 只记入「手动跳过」并继续下一条，既不进失败清单、
/// 也不写「无法建立连接」（design D4）。
///
/// 全部失败时把每个通道的失败原因拼成一条可读信息（界面直接展示，用户据此判断是限流、
/// 超时还是地址不可用）；`run` 的失败原因由调用方给出（含 HTTP 状态、超时等分类）。
async fn attempt_channels<T, F, Fut>(
    memory: &Mutex<ChannelMemory>,
    channels: &[Channel],
    mut on_attempt: impl FnMut(u32, u32, &Channel),
    mut run: F,
) -> Result<(T, Channel), InstallAbort>
where
    F: FnMut(&Channel) -> Fut,
    Fut: Future<Output = Result<T, AttemptError>>,
{
    let candidates = usable_channels(&memory.lock().unwrap(), channels);
    let total = candidates.len() as u32;
    let mut failures: Vec<String> = Vec::new();
    for (index, channel) in candidates.iter().enumerate() {
        let label = channel.label();
        // 每次尝试开始即记一行：日志里因此能重建「哪条通道、第几次、什么时候开始」的完整次序
        // （这是排查限流 / 黑洞超时时唯一有用的时间线）。
        logger::info(
            "env",
            format!("尝试下载通道 {}/{}：{label}", index + 1, total),
        );
        on_attempt(index as u32 + 1, total, channel);
        match run(channel).await {
            Ok(value) => return Ok((value, channel.clone())),
            Err(AttemptError::Canceled) => return Err(InstallAbort::Canceled),
            Err(AttemptError::Switched) => {
                logger::info(
                    "env",
                    format!("用户切换通道：跳过「{label}」（不计为失败）"),
                );
                // 「太慢」不是「连不上」：单独记一类，且不计入失败原因（design D4）。
                memory.lock().unwrap().mark_skipped(channel);
            }
            Err(AttemptError::Failure(failure)) => {
                logger::warn(
                    "env",
                    format!(
                        "通道「{label}」失败：{}{}",
                        failure.reason,
                        if failure.unreachable {
                            "（本次运行内不再尝试该通道）"
                        } else {
                            ""
                        }
                    ),
                );
                // 锁只在这一次极短临界区持有 —— 绝不跨网络请求持锁（design D5）。
                if failure.unreachable {
                    memory.lock().unwrap().mark_unreachable(channel);
                }
                failures.push(format!("{}：{}", channel.label(), failure.reason));
            }
        }
    }
    // 一条失败原因都没有 ⇒ 只会是「一路被用户手动跳过」。别拼成一句空的原因列表：
    // 那不是失败，得把真实原因说清楚。
    if failures.is_empty() {
        return Err(InstallAbort::AllFailed(format!(
            "本次下载未完成：可用通道都被你跳过了。{}",
            MANUAL_PLACE_HINT
        )));
    }
    Err(InstallAbort::AllFailed(format!(
        "全部下载通道均失败（{}）。{}",
        failures.join("；"),
        MANUAL_PLACE_HINT
    )))
}

/// 手动放置兜底提示（spec「自动获取失败」/ tasks 4.3）。
pub const MANUAL_PLACE_HINT: &str =
    "可在「更多」页点「打开目录」，手动把 scrcpy 与 adb 放入约定目录后重新检测。";

// ── HTTP ───────────────────────────────────────────────────

/// 单条通道的**连接**预算：超过即判该通道本次不可用，换下一条。
///
/// 只覆盖连接阶段（TCP + TLS，即 reqwest 的 `connect_timeout` 语义），**不覆盖收数据**：
/// 健康的通道下完 10.8 MiB 要几十秒，把整个请求限到 5s 会把正常下载也判死。
/// 实测本机官方直连 `github.com:443` 是 TCP 黑洞，OS 层要 ~21s 才放弃 —— 这个值把它压到 5s。
const CHANNEL_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

/// 「已建立连接但此后不再返回任何数据」的等待预算（design D3）。
///
/// reqwest 的 `read_timeout` 同时管两件事：**等响应头**（此前完全没有上限，最长挂到整体
/// 预算）与**响应体逐块空闲**（每次成功读后重置）。判据是「两次成功读之间的间隔」而非累计
/// 耗时，所以正常的涓流下载（哪怕只有 4 KB/s，数据一直在来）不会被误伤；命中的是真·僵死。
const CHANNEL_IDLE_TIMEOUT: Duration = Duration::from_secs(10);

/// 单条通道的**整体**预算（连接 + 收完响应体）：绝对值兜底。
///
/// 能烧到它的只剩「数据一直在来、但慢到十几分钟都下不完」——那属「极慢」而不是「僵死」，
/// 交给用户在界面上点「切换通道」处理（spec 的「下载过程的人工干预」）。
const CHANNEL_TOTAL_TIMEOUT: Duration = Duration::from_secs(900);

/// 构造 HTTP 客户端（async）：连接预算让「不可达的通道」快速失败并降级；
/// 停滞预算兜住「连上了但不再说话」；整体预算作绝对值兜底。
///
/// 为什么必须是 async 客户端：blocking `ClientBuilder` 在 reqwest 0.12 里**没有**
/// `read_timeout`，而且阻塞读无法从外部打断 —— 停滞降级与「点击即取消」两个诉求
/// 都只在 async 侧存在（design D1）。
fn http_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .connect_timeout(CHANNEL_CONNECT_TIMEOUT)
        .read_timeout(CHANNEL_IDLE_TIMEOUT)
        .timeout(CHANNEL_TOTAL_TIMEOUT)
        .user_agent("FreeControl2")
        .build()
        .map_err(|e| format!("初始化网络客户端失败：{}", e))
}

/// HTTP 状态码 → 可读原因（403/429 明确指向限流，便于用户理解降级缘由）。
fn describe_status(status: reqwest::StatusCode) -> String {
    let hint = match status.as_u16() {
        403 | 429 => "（可能被限流）",
        404 => "（资产不存在）",
        _ => "",
    };
    format!(
        "HTTP {}{}",
        status.as_u16(),
        if hint.is_empty() {
            String::new()
        } else {
            hint.to_string()
        }
    )
}

/// 网络错误 → 可读原因（超时 / 无法连接 / 其它）。
fn describe_error(err: &reqwest::Error) -> String {
    if err.is_timeout() {
        "请求超时".to_string()
    } else if err.is_connect() {
        "无法建立连接".to_string()
    } else if err.is_body() || err.is_decode() {
        "响应内容读取失败".to_string()
    } else {
        format!("网络错误：{}", err)
    }
}

/// 一次通道尝试的失败：可读原因 + 是否属于「无法建立连接」（design D2）。
///
/// 字符串化的失败原因分不出「连不上」与「被限流」，而后者正是通道记忆该不该写的唯一判据。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AttemptFailure {
    pub reason: String,
    /// `true` 表示该通道本次运行内已被判定为无法建立连接，后续请求应跳过它。
    pub unreachable: bool,
}

impl AttemptFailure {
    /// 已连上之后的失败、且手上**没有** `reqwest::Error`（HTTP 状态码、本地写入失败）：
    /// 通道本身可用，**不得**记入记忆（design D2）。
    fn body_reason(reason: String) -> Self {
        classify_failure(FailureStage::Body, false, false, reason)
    }
}

/// 失败发生在哪个阶段 —— 判据的分水岭（design D2）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FailureStage {
    /// 请求发出阶段（`send()`）：连接、TLS、重定向都在这里，失败可能意味着通道不可达。
    Send,
    /// 已连上之后的阶段：读到响应体即证明链路是通的，此处的失败属传输问题而非通道不可达。
    Body,
}

/// 按「阶段 + 错误种类」判定失败是否意味着通道不可达（design D2）。
///
/// 抽成纯函数是因为 `reqwest::Error` 无法在单测里构造：调用点只负责把
/// `err.is_connect()` / `err.is_timeout()` 传进来，判定逻辑本身可被完整覆盖。
fn classify_failure(
    stage: FailureStage,
    is_connect: bool,
    is_timeout: bool,
    reason: String,
) -> AttemptFailure {
    let unreachable = stage == FailureStage::Send && (is_connect || is_timeout);
    AttemptFailure { reason, unreachable }
}

/// `send()` 阶段失败 → 可能连不上（连接被拒 / 连接阶段超时）。
fn send_failure(err: &reqwest::Error) -> AttemptFailure {
    classify_failure(
        FailureStage::Send,
        err.is_connect(),
        err.is_timeout(),
        describe_error(err),
    )
}

/// 已连上之后的 `reqwest` 失败（读响应体中断等）。
///
/// 这里**照样把 `is_connect` / `is_timeout` 原样传进去**，由阶段把它们挡掉 —— 读到过响应体
/// 就证明链路是通的，此时的超时/连接标志只反映传输层中断，不构成「通道不可达」（design D2）。
/// 之所以不直接写死 `false`：那样阶段判定就成了摆设，读体路径一改就没人拦。
fn body_failure(err: &reqwest::Error) -> AttemptFailure {
    classify_failure(
        FailureStage::Body,
        err.is_connect(),
        err.is_timeout(),
        describe_error(err),
    )
}

/// 拉取文本内容（校验值汇总文件与 Releases API 共用）。
async fn fetch_text(client: &reqwest::Client, url: &str) -> Result<String, AttemptFailure> {
    let resp = client.get(url).send().await.map_err(|e| send_failure(&e))?;
    if !resp.status().is_success() {
        // 限流是瞬时的，官方 403 也不代表链路不通 ⇒ 不记入记忆。
        return Err(AttemptFailure::body_reason(describe_status(resp.status())));
    }
    resp.text().await.map_err(|e| body_failure(&e))
}

/// 在「一次单发请求」与「控制信号」之间择一就绪，两种命令都认（design D2）。
///
/// 取校验值这类单次 await 没法自己检查信号，只能由这里替它 race。
/// 外层 `Err` 是控制信号，内层 `Result` 是请求本身的结果 —— 分开返回是为了让调用方
/// 拿得到 `AttemptFailure`（它携带 `unreachable`，是通道记忆的唯一判据）。
async fn race_ctl<T, E, Fut>(ctl: &InstallCtl, fut: Fut) -> Result<Result<T, E>, AttemptError>
where
    Fut: Future<Output = Result<T, E>>,
{
    tokio::pin!(fut);
    loop {
        if let Some(cmd) = ctl.take_if_any() {
            return Err(ctl_abort(cmd));
        }
        tokio::select! {
            biased;
            _ = ctl.wait() => continue,
            r = &mut fut => return Ok(r),
        }
    }
}

/// `race_ctl` 的 `AttemptFailure` 版本：把请求失败直接并进 `AttemptError`（design D5）。
async fn race_attempt<T, Fut>(ctl: &InstallCtl, fut: Fut) -> Result<T, AttemptError>
where
    Fut: Future<Output = Result<T, AttemptFailure>>,
{
    match race_ctl(ctl, fut).await {
        Err(abort) => Err(abort),
        Ok(Err(failure)) => Err(AttemptError::Failure(failure)),
        Ok(Ok(value)) => Ok(value),
    }
}

/// 取校验值阶段的 race：**只认取消**（design D5）。
///
/// 「切换通道」在这个阶段没有意义 —— 文件已经下完，校验值只是一个小文本文件，换通道
/// 只会把 10.8 MiB 重下一遍；界面上也不在这个阶段提供它。真收到就地丢弃：
/// `take_if_any` 已经把它取走，因此也不会泄漏到后面的阶段。
async fn race_cancel<T, E, Fut>(ctl: &InstallCtl, fut: Fut) -> Result<Result<T, E>, AttemptError>
where
    Fut: Future<Output = Result<T, E>>,
{
    tokio::pin!(fut);
    loop {
        if let Some(CtlCmd::Cancel) = ctl.take_if_any() {
            return Err(AttemptError::Canceled);
        }
        tokio::select! {
            biased;
            _ = ctl.wait() => continue,
            r = &mut fut => return Ok(r),
        }
    }
}

/// 按候选名依次尝试获取官方校验值汇总文件（`SUMS_CANDIDATES`）。
///
/// 与资产同目录，故只依赖资产地址推导兄弟地址；全部候选都失败时返回逐条原因
/// —— 调用方据此判定「无法获取官方校验值」（tasks 4.5）。
async fn fetch_sums_with<F, Fut>(mut fetch: F) -> Result<String, String>
where
    F: FnMut(&str) -> Fut,
    Fut: Future<Output = Result<String, String>>,
{
    let mut failures: Vec<String> = Vec::new();
    for name in SUMS_CANDIDATES {
        match fetch(name).await {
            Ok(content) => return Ok(content),
            Err(reason) => failures.push(format!("{}：{}", name, reason)),
        }
    }
    Err(format!("官方校验值文件不可获取（{}）", failures.join("；")))
}

/// 下载到文件并回调进度（`total` 为 `None` 时表示服务端未给长度）。
///
/// 失败分类见 `FailureStage`：只有 `send()` 阶段的连接类失败才可能意味着通道不可达
/// （design D2）；停滞超时、读体中断与本地写入失败都不进通道记忆。
///
/// 请求阶段（连接 + 等响应头）与读取阶段（逐块）都接控制信号：
/// 前者由 `race_ctl` 包住 `send()`，后者在循环里 `select!`（design D1 / D2）。
/// 用户点「切换通道」或「取消」时立刻返回，不等本次读超时。
/// 循环每轮**先取一次命令字**：`Notify::notify_waiters` 不存 permit，点击可能落在
/// 「当前没有等待者」的窗口里（如上一块数据刚处理完的间隙）。
async fn download_to_file(
    client: &reqwest::Client,
    url: &str,
    dest: &Path,
    ctl: &InstallCtl,
    mut on_progress: impl FnMut(u64, Option<u64>),
) -> Result<u64, AttemptError> {
    let mut resp = match race_ctl(
        ctl,
        async { client.get(url).send().await.map_err(|e| send_failure(&e)) },
    )
    .await
    {
        Ok(Ok(resp)) => resp,
        // 控制信号先就绪：连接与等响应头期间也能被立刻打断（`read_timeout` 要 10s 才兜到）。
        Err(abort) => return Err(abort),
        Ok(Err(failure)) => return Err(AttemptError::Failure(failure)),
    };
    if !resp.status().is_success() {
        return Err(AttemptError::Failure(AttemptFailure::body_reason(
            describe_status(resp.status()),
        )));
    }
    let total = resp.content_length();
    let body = |reason: String| AttemptError::Failure(AttemptFailure::body_reason(reason));
    let mut file = fs::File::create(dest).map_err(|e| body(format!("创建临时文件失败：{}", e)))?;
    let mut done: u64 = 0;
    loop {
        if let Some(cmd) = ctl.take_if_any() {
            return Err(ctl_abort(cmd));
        }
        let chunk = tokio::select! {
            biased;
            _ = ctl.wait() => continue,
            chunk = resp.chunk() => chunk,
        };
        let chunk = chunk.map_err(|e| AttemptError::Failure(body_failure(&e)))?;
        // `None` 表示响应体结束。
        let Some(bytes) = chunk else {
            break;
        };
        file.write_all(&bytes)
            .map_err(|e| body(format!("写入临时文件失败：{}", e)))?;
        done += bytes.len() as u64;
        on_progress(done, total);
    }
    file.sync_all()
        .map_err(|e| body(format!("写入临时文件失败：{}", e)))?;
    Ok(done)
}

// ── 校验值（tasks 4.4 / 4.5） ──────────────────────────────

/// 从 `SHA256SUMS` 内容中定位某资产的 SHA-256。
///
/// 每行形如 `<64 位十六进制>  <文件名>`；GNU coreutils 会用 `*` 前缀标记二进制文件
/// （`*scrcpy-win64-v5.0.1.zip`），故按空白切分后需剥掉该前缀再比对。
pub fn parse_sha256sums(content: &str, asset: &str) -> Option<String> {
    for line in content.lines() {
        let mut parts = line.split_whitespace();
        let (Some(hash), Some(name)) = (parts.next(), parts.next()) else {
            continue;
        };
        if hash.len() != 64 || !hash.chars().all(|c| c.is_ascii_hexdigit()) {
            continue;
        }
        let name = name.trim_start_matches('*').trim_start_matches("./");
        if name == asset {
            return Some(hash.to_ascii_lowercase());
        }
    }
    None
}

/// 文件 SHA-256（小写十六进制）。
pub fn sha256_file(path: &Path) -> Result<String, String> {
    let mut file = fs::File::open(path).map_err(|e| format!("读取下载文件失败：{}", e))?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = file
            .read(&mut buf)
            .map_err(|e| format!("读取下载文件失败：{}", e))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex_lower(&hasher.finalize()))
}

fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

/// 校验结论（design D4 / tasks 4.4–4.5）。
#[derive(Debug, PartialEq, Eq)]
pub enum Verification {
    /// 官方校验值与本地一致 → 可继续
    Verified { expected: String },
    /// 官方校验值不匹配 → 拒绝解压与写入
    Mismatch { expected: String, actual: String },
    /// 无法获取官方校验值（拉取失败 / 缺少条目）→ 明确告知未校验，需用户确认
    Unverified { reason: String },
}

/// 由「拉取校验值的结果 + 本地哈希」判定结论（纯函数，便于覆盖 tasks 4.4 / 4.5 的四种情况）。
pub fn classify_verification(
    sums: Result<String, String>,
    asset: &str,
    actual: &str,
) -> Verification {
    let content = match sums {
        Ok(content) => content,
        Err(reason) => {
            return Verification::Unverified {
                reason: format!("无法获取官方校验值（{}）", reason),
            }
        }
    };
    match parse_sha256sums(&content, asset) {
        None => Verification::Unverified {
            reason: format!("官方 {} 中缺少 {} 的条目", SUMS_ASSET, asset),
        },
        Some(expected) if expected == actual => Verification::Verified { expected },
        Some(expected) => Verification::Mismatch {
            expected,
            actual: actual.to_string(),
        },
    }
}

// ── 临时目录与原子替换（tasks 4.6） ────────────────────────

/// 唯一临时目录名（进程内 + 时间戳，避免两次操作互相踩踏）。
fn unique_suffix() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{}-{}", std::process::id(), nanos)
}

/// 在应用数据根下新建临时目录（与目标目录同盘，保证 rename 原子）。
fn new_staging_dir(root: &Path) -> Result<PathBuf, String> {
    fs::create_dir_all(root).map_err(|e| format!("创建应用数据目录失败：{}", e))?;
    let dir = root.join(format!("{}{}", STAGING_PREFIX, unique_suffix()));
    fs::create_dir_all(&dir).map_err(|e| format!("创建临时目录失败：{}", e))?;
    Ok(dir)
}

fn remove_dir_quietly(path: &Path) {
    let _ = fs::remove_dir_all(path);
}

/// 清理上次运行残留的临时目录（tasks 4.3：失败不残留临时文件）。
///
/// 启动时调用：此时本进程没有在途下载，残留只可能来自上次异常退出。
/// 顺带清理替换时留下的旧目录备份（仅在其删除失败时才会残留）。
pub fn cleanup_stale_staging(root: &Path) {
    if let Ok(entries) = fs::read_dir(root) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            let is_staging = name.starts_with(STAGING_PREFIX);
            let is_backup = name.starts_with(&format!(".{}", scrcpy::DIR_PREFIX))
                && name.contains("-backup-");
            if is_staging || is_backup {
                remove_dir_quietly(&entry.path());
            }
        }
    }
}

/// 临时目录守卫：未显式「保留」时，离开作用域即删除 —— 任何失败路径都不会留下临时文件。
struct StagingGuard {
    path: PathBuf,
    keep: bool,
}

impl StagingGuard {
    fn new(path: PathBuf) -> Self {
        StagingGuard { path, keep: false }
    }
    fn keep(&mut self) {
        self.keep = true;
    }
}

impl Drop for StagingGuard {
    fn drop(&mut self) {
        if !self.keep {
            remove_dir_quietly(&self.path);
        }
    }
}

/// 解压入口：按目标平台的压缩后缀分派到具体实现（design D2）。
///
/// 回调的两个参数是「已解压条目数」与「总条目数」—— 后者未知时为 `None`（见 [`extract_tar_gz`]）。
pub fn extract_archive(
    archive: &Path,
    platform: &scrcpy::Platform,
    dest: &Path,
    on_entry: impl FnMut(usize, Option<usize>),
) -> Result<(), String> {
    match platform.ext {
        "zip" => extract_zip(archive, dest, on_entry),
        "tar.gz" => extract_tar_gz(archive, dest, on_entry),
        other => Err(format!("不支持的压缩格式：{other}")),
    }
}

/// 解压 zip 到 `dest`；顺带回调「已解压条目数 / 总条目数」用于进度。
///
/// 逐条校验路径落在 `dest` 内（防目录穿越），目录条目直接建目录。
pub fn extract_zip(
    zip_path: &Path,
    dest: &Path,
    mut on_entry: impl FnMut(usize, Option<usize>),
) -> Result<(), String> {
    let file = fs::File::open(zip_path).map_err(|e| format!("打开压缩包失败：{}", e))?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|e| format!("压缩包无法解析（可能下载不完整）：{}", e))?;
    let total = archive.len();
    fs::create_dir_all(dest).map_err(|e| format!("创建解压目录失败：{}", e))?;
    for i in 0..total {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| format!("读取压缩包条目失败：{}", e))?;
        let rel = entry
            .enclosed_name()
            .ok_or_else(|| format!("压缩包内含非法路径：{}", entry.name()))?
            .to_path_buf();
        let out = dest.join(&rel);
        if entry.is_dir() {
            fs::create_dir_all(&out).map_err(|e| format!("创建目录失败：{}", e))?;
        } else {
            if let Some(parent) = out.parent() {
                fs::create_dir_all(parent).map_err(|e| format!("创建目录失败：{}", e))?;
            }
            let mut writer =
                fs::File::create(&out).map_err(|e| format!("解压写入失败（{}）：{}", rel.display(), e))?;
            std::io::copy(&mut entry, &mut writer)
                .map_err(|e| format!("解压写入失败（{}）：{}", rel.display(), e))?;
            writer
                .sync_all()
                .map_err(|e| format!("解压写入失败（{}）：{}", rel.display(), e))?;
        }
        on_entry(i + 1, Some(total));
    }
    Ok(())
}

/// 归档条目的路径是否会越出解压目录（绝对路径，或以 `..` 上跳）。
///
/// 与 zip 分支的 `enclosed_name` 是同一道防线。抽成纯函数是为了能直接断言它 ——
/// 压不出「含 `..` 的归档」来测（`tar` 的 Builder 自己就拒绝写这样的条目）。
fn escapes_dest(path: &Path) -> bool {
    path.is_absolute()
        || path
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
}

/// 解压 `.tar.gz` 到 `dest`（Linux 官方资产格式）；顺带回调「已解压条目数 / 总数未知」。
///
/// 与 zip 分支的两点差异，都是 Linux 上必须成立的：
/// 1. **权限位必须保留**：归档里的 `scrcpy` 与 `adb` 带可执行位，而手写「读条目 → 建文件 →
///    copy」会把它丢掉，失效方式是「运行环境判定就绪、一跑就失败」。故走 tar 的
///    `unpack_in`，由它按条目头落权限（zip 分支之所以能手写，是因为 Windows 没有权限位这回事）。
/// 2. **总数不可知**：gzip 流里没有条目计数，要拿到总数就得先把整个流解一遍。故回调的第二项
///    为 `None`，界面文案退化为「已解压 N 项」（design D2）。
pub fn extract_tar_gz(
    archive_path: &Path,
    dest: &Path,
    mut on_entry: impl FnMut(usize, Option<usize>),
) -> Result<(), String> {
    let file = fs::File::open(archive_path).map_err(|e| format!("打开压缩包失败：{}", e))?;
    let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(file));
    fs::create_dir_all(dest).map_err(|e| format!("创建解压目录失败：{}", e))?;

    let entries = archive
        .entries()
        .map_err(|e| format!("压缩包无法解析（可能下载不完整）：{}", e))?;
    let mut done = 0usize;
    for entry in entries {
        let mut entry = entry.map_err(|e| format!("读取压缩包条目失败：{}", e))?;
        // 先自己把非法路径拦下（绝对路径、含 `..`），再交给 `unpack_in` —— 与 zip 分支的
        // `enclosed_name` 是同一道防线，两道都留着：一道给出可读的错误，一道兜底。
        let path = entry
            .path()
            .map_err(|e| format!("读取压缩包条目失败：{}", e))?
            .into_owned();
        if escapes_dest(&path) {
            return Err(format!("压缩包内含非法路径：{}", path.display()));
        }
        entry
            .unpack_in(dest)
            .map_err(|e| format!("解压写入失败（{}）：{}", path.display(), e))?;
        done += 1;
        on_entry(done, None);
    }
    Ok(())
}

/// 在解压结果中定位「真正含 scrcpy 主程序的目录」。
///
/// 官方资产解压后会多一层 `scrcpy-win64-v{Version}/`，但也兼容直接铺开（或再深一层）的包，
/// 因此按「含 scrcpy 主程序」判定，最多下探两层。
pub fn find_payload_root(raw: &Path) -> Option<PathBuf> {
    const MAX_DEPTH: u8 = 2;

    fn walk(dir: &Path, depth: u8) -> Option<PathBuf> {
        if dir.join(scrcpy::SCRCPY_BIN).is_file() {
            return Some(dir.to_path_buf());
        }
        if depth == 0 {
            return None;
        }
        let entries = fs::read_dir(dir).ok()?;
        for entry in entries.flatten() {
            if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                if let Some(found) = walk(&entry.path(), depth - 1) {
                    return Some(found);
                }
            }
        }
        None
    }

    walk(raw, MAX_DEPTH)
}

/// 原子替换目录：先把旧目录改名为备份，再把新目录改名到位；后者失败则回滚。
///
/// 同盘 rename 在 Windows 上为原子操作，因此「失败不破坏现状」成立（design D4）。
pub fn replace_dir(payload: &Path, target: &Path) -> Result<(), String> {
    let parent = target
        .parent()
        .ok_or_else(|| "运行环境目录路径异常".to_string())?;
    let name = target
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "scrcpy".to_string());
    let backup = parent.join(format!(".{}-backup-{}", name, unique_suffix()));
    let had_target = target.exists();

    if had_target {
        fs::rename(target, &backup).map_err(|e| {
            format!(
                "无法替换运行环境目录（旧目录被占用？请先断开所有连接）：{}",
                e
            )
        })?;
    }
    match fs::rename(payload, target) {
        Ok(()) => {
            if had_target {
                remove_dir_quietly(&backup);
            }
            Ok(())
        }
        Err(e) => {
            // 回滚：把备份改回原名，保证原有运行环境依旧可用。
            if had_target {
                let _ = fs::rename(&backup, target);
            }
            Err(format!("写入运行环境目录失败：{}", e))
        }
    }
}

// ── 占用检测与释放（design D2 / D6） ──────────────────

/// 运行环境目录内第一个被其它进程占用的文件。
///
/// 判定方式是「以独占共享模式打开」：正在运行的可执行映像会被系统持有句柄，
/// 此时独占打开必然失败（`ERROR_SHARING_VIOLATION` / `ERROR_LOCK_VIOLATION`）。
/// 只把这两种错误视为「占用」，避免把权限不足、文件不存在等误判成活跃进程。
///
/// 返回文件名而不是「有没有」：替换失败时要把这个文件名写进提示（design D6）——
/// 只说「目录被占用」用户无从下手，点名文件才能让他去关掉那个程序。
pub fn occupied_file(dir: &Path) -> Option<PathBuf> {
    let entries = fs::read_dir(dir).ok()?;
    entries
        .flatten()
        .filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false))
        .map(|e| e.path())
        .find(|p| file_in_use(p))
}

#[cfg(windows)]
fn file_in_use(path: &Path) -> bool {
    use std::os::windows::fs::OpenOptionsExt;
    const ERROR_SHARING_VIOLATION: i32 = 32;
    const ERROR_LOCK_VIOLATION: i32 = 33;

    match fs::OpenOptions::new()
        .write(true)
        .share_mode(0)
        .open(path)
    {
        Ok(_) => false,
        Err(e) => matches!(
            e.raw_os_error(),
            Some(ERROR_SHARING_VIOLATION) | Some(ERROR_LOCK_VIOLATION)
        ),
    }
}

#[cfg(not(windows))]
fn file_in_use(path: &Path) -> bool {
    // **有意的降级，不是未实现**：类 Unix 平台没有「独占打开」语义，且不需要它 ——
    // 见 `occupancy_blocks_replace` 的理由。故这里恒为「未占用」。
    let _ = path;
    false
}

/// 本平台是否会因「运行环境内仍有进程占用」而拒绝替换（tasks 2.5）。
///
/// Windows：运行中的可执行映像被系统持有句柄，独占打开必然失败 —— 因此**必须**先检测并处理
/// 占用（先结束本应用拉起的 adb 服务端，仍占用则点名文件拒绝），否则替换会失败在半途
/// （`replace_dir` 的 rename 会报「旧目录被占用」）。
///
/// 类 Unix（含 Linux 与将来的 macOS）：没有这个约束，也不需要这份检测 —— 它们允许改名目录、
/// 允许 unlink 已打开的文件（inode 一直存活到最后一个句柄关闭），因此替换正在使用的运行环境
/// 对运行中的会话无害：现有会话继续用旧 inode，新会话用新文件。
///
/// 于是非 Windows 上「替换正在使用的运行环境」**不会静默破坏现状**（这正是 tasks 2.5 要的保证），
/// 代价是不会再多给一次「请先断开」的人情提示。
pub const fn occupancy_blocks_replace() -> bool {
    cfg!(windows)
}

/// 替换前的占用检查：目录内仍有文件被占用时不替换，并点名那个文件（design D6）。
///
/// **不再**声称「可能有 scrcpy 会话未断开」：本应用会话在运行的情形由
/// [`ensure_sessions_stopped`] 单独判定并给出准确提示 —— 这里剩下的只可能是别的进程
/// （最常见的是本应用的探测拉起、却尚未被 [`release_adb_server`] 结束掉的 adb 服务端）。
fn ensure_dir_free(dir: &Path) -> Result<(), String> {
    if !occupancy_blocks_replace() {
        // 本平台替换正在使用的目录本身就是安全的（见 `occupancy_blocks_replace`）。
        return Ok(());
    }
    match occupied_file(dir) {
        Some(path) => Err(format!(
            "运行环境目录内仍有进程在使用：{} 被占用，无法替换。请关闭占用它的程序后重试。",
            file_label(&path)
        )),
        None => Ok(()),
    }
}

/// 路径中用于提示的文件名（取不到文件名时退回完整路径）。
fn file_label(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| path.display().to_string())
}

/// 开始替换前是否允许继续：本应用有会话在运行时必须先断开（design D1）。
///
/// 判据是**本应用会话表**，不是目录内的文件占用 —— 目录被 adb 服务端占用是常态
/// （它由本应用的设备状态探测拉起），把它当成「有会话未断开」会让用户在没有任何
/// 连接可断的情况下被拦死（这正是本次要修的缺陷）。反过来，真有会话在运行时，
/// 这句「请先断开」指向的是界面上一个真实可点的操作。
fn ensure_sessions_stopped(active: &HashSet<String>) -> Result<(), String> {
    if active.is_empty() {
        return Ok(());
    }
    Err(format!(
        "仍有 {} 台设备的会话在运行，请先断开连接再重新下载。",
        active.len()
    ))
}

/// 结束 adb 服务端的超时预算。
///
/// 正常情况毫秒级返回（`kill-server` 只是让服务端退出）；给 5 秒是为了让「服务端卡死」
/// 不至于把一次安装拖成无响应 —— 超时即视为本次释放失败，随后仍会走占用检查，
/// 由那次检查给出点名文件的可读原因。
pub const ADB_KILL_TIMEOUT: Duration = Duration::from_secs(5);

/// 结束运行环境目录内的 adb 服务端；`Ok(true)` 表示确实执行了结束动作。
///
/// 为什么需要：该服务端由本应用的设备状态探测拉起（`adb devices` 在服务端缺失时会
/// fork 一个常驻服务端，见 `device::probe_adb`），其可执行映像就是目录内的 `adb.exe`，
/// 因此它会一直持有句柄、让替换无法进行，而它不属于任何会话 —— 用户没有任何界面操作
/// 能让它退出（design D3）。
///
/// 平台分流：本平台若不因进程占用而拒绝替换（见 [`occupancy_blocks_replace`]），
/// 也就没有理由去干扰一个与其它工具共享的 adb 服务端 —— 此时直接返回 `Ok(false)`。
pub fn release_adb_server(dir: &Path) -> Result<bool, String> {
    if !occupancy_blocks_replace() {
        return Ok(false);
    }
    run_adb_kill_server(&dir.join(assets::ADB_BIN), &["kill-server"], ADB_KILL_TIMEOUT)?;
    Ok(true)
}

/// 由二进制与参数执行结束动作。拆开是为了让单测用系统命令覆盖「成功 / 超时 / 非零退出」
/// 三种返回 —— 真实 adb 只在用户机器上存在，单测无法依赖它。
fn run_adb_kill_server(adb: &Path, args: &[&str], timeout: Duration) -> Result<(), String> {
    if !adb.is_file() {
        return Err(format!(
            "运行环境内未找到 {}，无法结束 adb 服务端",
            file_label(adb)
        ));
    }
    device::run_captured(adb, args, timeout).map(|_| ())
}

/// 替换目录前的最后一步（design D2）：先确认没有本应用会话在运行，再结束 adb 服务端，
/// 最后确认目录可替换。
///
/// 释放失败**不致命**：它只是让下一步的占用检查更可能通过；真的没通过时，由那次检查给出
/// 点名文件的原因，而不是在这里把安装打断在一句对用户无意义的「释放失败」上。
fn prepare_replace(active: &HashSet<String>, dir: &Path) -> Result<(), String> {
    ensure_sessions_stopped(active)?;
    match release_adb_server(dir) {
        // 只记「确实做了」：`Ok(false)` 是本平台不需要释放，不是失败。
        Ok(true) => logger::info("env", "已在替换前结束运行环境内的 adb 服务端"),
        Ok(false) => {}
        Err(e) => logger::warn("env", format!("结束 adb 服务端失败：{e}（继续按占用检查判定）")),
    }
    ensure_dir_free(dir)
}

// ── 下载目标解析（best-fit / 官方最新） ────────────────────

/// 本次下载的目标资产。
#[derive(Clone, Debug)]
pub struct Target {
    /// 目标版本
    pub version: String,
    /// 资产文件名（`SHA256SUMS` 中按它定位条目）
    pub asset: String,
    /// 官方原始下载链接
    pub url: String,
    /// 需要向用户说明的降级/回退情况（如 API 查询失败后回退）
    pub note: Option<String>,
}

/// 最佳适配版本的目标资产（首启自动获取用；不依赖任何 API）。
///
/// 当前平台不在受支持表内时返回 `Err`：资产名由平台标签与压缩格式决定，取不出来就不该开始下载。
pub fn best_fit_target() -> Result<Target, String> {
    Ok(best_fit_target_at(scrcpy::current()?))
}

/// 按给定平台拼最佳适配版本的目标资产（纯函数，便于在任一平台上覆盖两行取值）。
pub fn best_fit_target_at(platform: &scrcpy::Platform) -> Target {
    let version = scrcpy::BEST_FIT_VERSION.to_string();
    let asset = asset_at(platform, &version);
    Target {
        url: format!("{}/v{}/{}", RELEASES_BASE, version, asset),
        version,
        asset,
        note: None,
    }
}

/// GitHub Releases JSON → 最新版本资产（`tag_name` + **本平台**的发布资产）。
///
/// 匹配按当前平台的标签与压缩后缀进行；资产名优先采用发布数据里的写法（官方若调整命名也能跟上），
/// 该条目缺下载地址时按约定拼地址。
///
/// IF 发布数据里**没有本平台的资产**，THEN 返回 `Err` 而不是回退：回退只会拿别的平台的资产名去试，
/// 表现为「装上了但跑不起来」，比明确报错难查得多（spec `scrcpy-environment`「按当前平台匹配发布资产」）。
pub fn parse_latest_release(body: &str) -> Result<Target, String> {
    let json: serde_json::Value =
        serde_json::from_str(body).map_err(|e| format!("发布信息解析失败：{}", e))?;
    let tag = json
        .get("tag_name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "发布信息中缺少版本号".to_string())?;
    let version = tag.trim_start_matches('v').to_string();
    if version.is_empty() {
        return Err("发布信息中的版本号为空".to_string());
    }

    let platform = scrcpy::current()?;
    let prefix = format!("scrcpy-{}-v", platform.tag);
    let suffix = format!(".{}", platform.ext);
    let matched = json
        .get("assets")
        .and_then(|a| a.as_array())
        .and_then(|assets| {
            assets.iter().find_map(|asset| {
                let name = asset.get("name")?.as_str()?;
                // 资产名会被直接当作本地文件名使用，故拒绝任何带路径分隔符的写法。
                let safe = !name.contains('/') && !name.contains('\\');
                if safe && name.starts_with(&prefix) && name.ends_with(&suffix) {
                    Some((
                        name.to_string(),
                        asset
                            .get("browser_download_url")
                            .and_then(|u| u.as_str())
                            .map(|u| u.to_string()),
                    ))
                } else {
                    None
                }
            })
        });

    let (asset, url) = match matched {
        Some((name, Some(url))) => (name, url),
        // 资产名存在但没有下载地址 → 按约定拼地址（平台标签与后缀都已确定，拼出来仍是本平台的包）。
        Some((name, None)) => {
            let url = format!("{}/v{}/{}", RELEASES_BASE, version, name);
            (name, url)
        }
        None => {
            return Err(format!(
                "官方发布 v{} 中没有本平台的发布包（应为 {}），未开始下载。",
                version,
                asset_at(platform, &version)
            ));
        }
    };

    Ok(Target {
        version,
        asset,
        url,
        note: None,
    })
}

/// 解析本次要下载的目标：`latest=true` 走官方最新发布，失败则回退最佳适配（并附说明）。
///
/// 第二项是「**本流程内已确认可用**的通道」（design D4 / E1）：`latest` 模式下由 Releases API
/// 的查询结果给出，随后的资产下载据此把该通道排到最前，不再从通道列表开头重走一遍；
/// `bestFit` 不查询 API，故为 `None`。
///
/// 查询最新版本同样走下载通道（官方 → 配置里的代理地址），因此在官方接口被墙但代理可用时
/// 「下载最新版本」仍能拿到官方最新版本号；失败则回退到确定性的最佳适配版本，
/// 并把原因随结果一并告知用户（不做静默切换）。**用户取消例外** —— 那不叫失败（design D6）。
async fn resolve_target(
    channels: &[Channel],
    client: &reqwest::Client,
    latest: bool,
    memory: &Mutex<ChannelMemory>,
    ctl: &InstallCtl,
    on_attempt: impl FnMut(u32, u32, &Channel),
) -> Result<(Target, Option<Channel>), InstallAbort> {
    if !latest {
        return Ok((best_fit_target().map_err(InstallAbort::AllFailed)?, None));
    }
    let queried = attempt_channels(memory, channels, on_attempt, |channel| {
        let url = channel.url_for(LATEST_RELEASE_API);
        // 在 async 块**外面**取好引用：块内直接 `async move` 会尝试把闭包捕获的
        // 非 Copy 值（如 `Client`）移出，而 `FnMut` 闭包不允许「移出捕获项」。
        let client = &client;
        // 查询阶段也接控制信号：用户此刻点「取消」应当立刻停下，而不是等这次请求超时。
        async move { race_attempt(ctl, fetch_text(client, &url)).await }
    })
    .await;
    target_from_api(queried)
}

/// 由「Releases API 查询结果」决定目标与已确认可用通道（纯函数，便于单测）。
///
/// 取消要**原样上抛**：把它当成「查询失败」静默回退到最佳适配版本，会让用户点了取消
/// 却看见一次正常开始的下载（design D6）。
fn target_from_api(
    queried: Result<(String, Channel), InstallAbort>,
) -> Result<(Target, Option<Channel>), InstallAbort> {
    // 回退本身也可能失败（当前平台不在受支持表内）—— 那种情况下不再“回退”，直接把原因上抛。
    let fallback = |reason: String| -> Result<(Target, Option<Channel>), InstallAbort> {
        let mut target = best_fit_target().map_err(InstallAbort::AllFailed)?;
        target.note = Some(format!(
            "无法查询官方最新版本（{}），本次改为下载最佳适配版本 v{}。",
            reason, target.version
        ));
        Ok((target, None))
    };
    match queried {
        Err(InstallAbort::Canceled) => Err(InstallAbort::Canceled),
        Err(InstallAbort::AllFailed(reason)) => fallback(reason),
        Ok((body, channel)) => match parse_latest_release(&body) {
            Ok(target) => Ok((target, Some(channel))),
            Err(reason) => fallback(reason),
        },
    }
}

/// 把「本流程内已确认可用」的通道挪到最前，其余保持原顺序（design D4 / E1）。
///
/// `first` 不在 `channels` 里时原样返回 —— 长度恒定，界面上的「N/M」才不会与实际会尝试的
/// 通道数脱节。
fn prioritize(channels: &[Channel], first: Option<&Channel>) -> Vec<Channel> {
    match first {
        Some(first) if channels.contains(first) => {
            let mut list = Vec::with_capacity(channels.len());
            list.push(first.clone());
            list.extend(channels.iter().filter(|c| *c != first).cloned());
            list
        }
        _ => channels.to_vec(),
    }
}

// ── 安装流程 ───────────────────────────────────────────────

/// 进度事件载荷。
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallProgress {
    /// "download" | "verify" | "extract" | "replace" | "done"
    pub phase: String,
    /// 0–100；服务端未提供长度时保持 0
    pub percent: u8,
    /// 面向用户的文案（含当前通道）
    pub text: String,
    /// **已生效（成功）的通道**标签；尚未确定时为空串
    pub channel: String,
    /// 本次尝试的序号（从 1 开始）；0 表示该事件不带尝试信息
    pub attempt: u32,
    /// 本次可用通道总数；0 表示该事件不带尝试信息
    pub attempt_total: u32,
    /// 正在尝试的通道标签；空串表示该事件不带尝试信息
    pub attempt_channel: String,
}

impl InstallProgress {
    /// 普通进度事件（不带尝试信息）。
    fn plain(phase: &str, percent: u8, text: String, channel: &str) -> Self {
        InstallProgress {
            phase: phase.to_string(),
            percent,
            text,
            channel: channel.to_string(),
            attempt: 0,
            attempt_total: 0,
            attempt_channel: String::new(),
        }
    }

    /// 「正在尝试通道 N/M」事件：在每次尝试**之前**发出（design D3）。
    ///
    /// `text` 与 `channel` 都留空，且都是有意的：
    /// - `text` 空 —— 这句话里含「已耗时秒数」，只有前端算得出来；`attempt` 等字段已经
    ///   把「第几条 / 共几条 / 哪条通道」交代清楚，后端再拼一句就会与前端那句**并排重复**
    ///   （两者现在渲染在同一行）。
    /// - `channel` 空 —— 它只承载「已生效」的通道，若拿它承载「正在尝试」，
    ///   一次失败的尝试就会污染「本次走的是哪条通道」的展示。
    fn attempt(index: u32, total: u32, channel: &Channel) -> Self {
        InstallProgress {
            phase: "download".to_string(),
            // 换一条通道即从头下起，进度随之复位。
            percent: 0,
            text: String::new(),
            channel: String::new(),
            attempt: index,
            attempt_total: total,
            attempt_channel: channel.label(),
        }
    }
}

/// 一次安装的结果。
///
/// `status` 取值：
/// - `"done"`：已完成（含未经校验但用户已确认的）；
/// - `"needsConfirmation"`：等待用户对「未经校验」确认；
/// - `"canceled"`：用户在下载/校验阶段点了「取消」（design D6）。
///
/// 取消之所以是一个**结论**而不是 `Err`：`installError` 在界面上是红色的失败原因，
/// 用户主动取消不属于失败 —— 写进去既会把一次正常操作染成红色，又会掩盖上一次真实失败。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallOutcome {
    pub status: String,
    pub version: String,
    pub asset: String,
    pub channel: String,
    pub channel_url: String,
    pub verified: bool,
    pub unverified_reason: Option<String>,
    pub note: Option<String>,
}

impl InstallOutcome {
    /// 「用户取消」的结论：除状态外全部留空，界面据此收起进度块、不报错（design D6）。
    pub fn canceled() -> Self {
        InstallOutcome {
            status: "canceled".to_string(),
            version: String::new(),
            asset: String::new(),
            channel: String::new(),
            channel_url: String::new(),
            verified: false,
            unverified_reason: None,
            note: None,
        }
    }
}

/// 等待用户确认「未经校验继续安装」的中间态（保留已下载文件，避免重复下载）。
pub struct PendingInstall {
    staging: PathBuf,
    target: Target,
    channel: Channel,
    channel_url: String,
    /// 已下载的压缩包（Windows 为 `.zip`、Linux 为 `.tar.gz`，故不叫 `zip`）
    archive: PathBuf,
    unverified_reason: String,
}

impl PendingInstall {
    fn into_outcome(&self) -> InstallOutcome {
        InstallOutcome {
            status: "needsConfirmation".to_string(),
            version: self.target.version.clone(),
            asset: self.target.asset.clone(),
            channel: self.channel.label(),
            channel_url: self.channel_url.clone(),
            verified: false,
            unverified_reason: Some(self.unverified_reason.clone()),
            note: self.target.note.clone(),
        }
    }
}

/// 把**阻塞工作**放到阻塞线程池上执行（解压、原子替换，以及 `session` 的进程控制）。
///
/// 历史原因（现已不成立）：这里原先还包着整段下载，因为**阻塞版**客户端内部持有一个
/// tokio 运行时、**在异步上下文中析构会 panic**（"Cannot drop a runtime in a context where
/// blocking is not allowed"）。网络路径迁到 async 之后（design D1）客户端不再自带运行时，
/// 那条限制随之消失，代码里也不再有任何阻塞版客户端。
///
/// 保留它的理由变成另一件事：解压与原子替换是同步 FS + `zip` 解码，会把 tokio 的工作
/// 线程堵住较长时间 —— 那正是阻塞线程池该承担的活。阻塞任务内的 panic 会以 `Err` 回传
/// （而不是把进程带走），错误信息里带上原因。
pub(crate) async fn run_blocking<T, F>(task: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, String> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(task)
        .await
        .map_err(|e| format!("后台任务执行失败：{}", e))?
}

fn emit_progress(app: &AppHandle, phase: &str, percent: u8, text: String, channel: &str) {
    let _ = app.emit(
        PROGRESS_EVENT,
        InstallProgress::plain(phase, percent, text, channel),
    );
}

/// 每条通道尝试**之前**发出的事件（design D3）：界面据此显示
/// 「正在尝试通道 N/M：<标签>」，连接阶段与黑洞超时期间不再只有一句占位文案。
fn emit_attempt(app: &AppHandle, index: u32, total: u32, channel: &Channel) {
    let _ = app.emit(PROGRESS_EVENT, InstallProgress::attempt(index, total, channel));
}

/// 取出并丢弃待确认的安装（清掉其中的临时文件）。
fn discard_pending(state: &State<'_, AppState>) {
    let taken = state.pending.lock().unwrap().take();
    if let Some(pending) = taken {
        remove_dir_quietly(&pending.staging);
    }
}

fn proxy_addresses(state: &State<'_, AppState>) -> Vec<String> {
    state.config.lock().unwrap().channels.proxies.clone()
}

/// 读取在途安装的控制入口（design D2）。
fn install_ctl(state: &State<'_, AppState>) -> Option<Arc<InstallCtl>> {
    state.install.lock().unwrap().clone()
}

/// 一次获取的失败出口：取消不算错误（design D6）。
#[derive(Debug)]
enum InstallError {
    Canceled,
    Failed(String),
}

/// 把「流程被中止」翻译成失败出口（design D6）。
fn abort_to_error(abort: InstallAbort) -> InstallError {
    match abort {
        InstallAbort::Canceled => InstallError::Canceled,
        InstallAbort::AllFailed(reason) => InstallError::Failed(reason),
    }
}

/// 下载 + 校验阶段；返回待安装（未经校验时需要用户确认）或已处理完的结论。
///
/// `ctl` 是本次获取的控制入口（design D2）：它在整个流程内有效，用户随时可以
/// 「切换通道」或「取消」，两条路径都在这里被翻译成 `InstallError` 或「换下一条」。
async fn download_and_verify(
    app: &AppHandle,
    state: &State<'_, AppState>,
    latest: bool,
    ctl: Arc<InstallCtl>,
) -> Result<InstallOutcome, InstallError> {
    let channels = resolve_channels(&proxy_addresses(state));
    let client = http_client().map_err(InstallError::Failed)?;
    // E1：Releases API 阶段已选出的通道优先 —— 资产下载与校验值获取据此复用，
    // 不再从通道列表开头重走一遍（design D4）。
    let (target, discovered) = resolve_target(
        &channels,
        &client,
        latest,
        &state.channel_memory,
        &ctl,
        |index, total, channel| emit_attempt(app, index, total, channel),
    )
    .await
    .map_err(abort_to_error)?;
    let channels = prioritize(&channels, discovered.as_ref());
    // 获取开始的上下文：目标版本 + 资产名 + 本次可用通道数。紧接着的「尝试下载通道 1/N」
    // 就是本次的实际首条通道 —— 这样两行合起来正好是「目标」与「首个通道」两件事，
    // 不必在这一行里猜一条稍后才会被记忆过滤掉的通道。
    logger::info(
        "env",
        format!(
            "开始获取运行环境：目标 v{}（{}，可用通道 {} 条）",
            target.version,
            target.asset,
            channels.len()
        ),
    );
    if let Some(note) = &target.note {
        // 「最新版本查询失败 → 回退最佳适配」是有意为之的行为，必须能从日志里看出它发生过。
        logger::warn("env", format!("下载目标回退：{note}"));
    }

    let root = config::app_data_root(app).map_err(InstallError::Failed)?;
    let guard = {
        let staging = new_staging_dir(&root).map_err(InstallError::Failed)?;
        StagingGuard::new(staging)
    };
    let staging = guard.path.clone();
    let archive_path = staging.join(&target.asset);

    emit_progress(
        app,
        "download",
        0,
        format!("正在准备 {}…", target.asset),
        "",
    );

    // ① 下载：GitHub 直连 → 配置里的代理地址依次降级；已确认可用的通道已排到最前（E1）。
    let (_, channel) = attempt_channels(
        &state.channel_memory,
        &channels,
        |index, total, channel| emit_attempt(app, index, total, channel),
        |channel| {
            let url = channel.url_for(&target.url);
            // 标签在 async 块**外面**算好：块内若借用闭包的 `&Channel` 参数，
            // 返回的 future 就会带上「本次调用」的生命周期，不再是同一个类型。
            let label = channel.label();
            // 同样在块外取好引用：`async move` 会把捕获项整体移入，而 `FnMut` 闭包
            // 不允许移出非 Copy 的捕获项（`Client` / `PathBuf` / `Arc` 都不是 Copy）。
            let client = &client;
            let archive_path = &archive_path;
            let ctl = &ctl;
            let target = &target;
            // 进度去重是**每次尝试各自**的：换一条通道即从头下起，上一次的百分比不参与比较。
            let mut last_percent: i32 = -1;
            async move {
                download_to_file(client, &url, archive_path, ctl, |done, total| {
                    let percent = match total {
                        Some(t) if t > 0 => ((done * 100) / t) as i32,
                        _ => -1,
                    };
                    if percent == last_percent {
                        return;
                    }
                    last_percent = percent;
                    let text = match total {
                        // 文案里**不带百分比**：`percent` 由同一行的百分格单独展示，
                        // 两处都写会在同一行重复（design D8 的「不重复」原则，
                        // 也是让文案 + 通道 + 百分比能放进一行所必需的）。
                        Some(t) if t > 0 => format!(
                            "正在下载 {}（{} / {}）…",
                            target.asset,
                            humansize(done),
                            humansize(t)
                        ),
                        _ => format!("正在下载 {}（已下载 {}）…", target.asset, humansize(done)),
                    };
                    emit_progress(
                        app,
                        "download",
                        if percent < 0 { 0 } else { percent as u8 },
                        text,
                        &label,
                    );
                })
                .await
            }
        },
    )
    .await
    .map_err(abort_to_error)?;

    // ② 校验：从**本次生效的通道**取官方校验值汇总文件（design D4 / tasks 4.4）。
    // 这一阶段只接受「取消」——文件已经下完，切换通道只会把资产重下一遍（见 `race_cancel`）。
    let channel_url = channel.url_for(&target.url);
    emit_progress(
        app,
        "verify",
        100,
        format!("正在校验 {} 的 SHA-256…", target.asset),
        &channel.label(),
    );
    let sums = match race_cancel(
        &ctl,
        fetch_sums_with(|name| {
            let url = sibling_url(&target.url, name);
            // 同上下载闭包：引用在块外取好，块内只搬运 Copy 的引用。
            let client = &client;
            let channel = &channel;
            async move {
                match url {
                    None => Err("无法推导校验值地址".to_string()),
                    // 校验值获取发生在某条通道**已经成功**之后，其失败不构成「通道不可达」
                    // 信号（design D2），故这里只取出可读原因。
                    Some(url) => fetch_text(client, &channel.url_for(&url))
                        .await
                        .map_err(|failure| failure.reason),
                }
            }
        }),
    )
    .await
    {
        // `race_cancel` 只可能因取消而提前返回。
        Err(_) => return Err(InstallError::Canceled),
        Ok(sums) => sums,
    };
    let local_hash = sha256_file(&archive_path).map_err(InstallError::Failed)?;

    match classify_verification(sums, &target.asset, &local_hash) {
        Verification::Mismatch { expected, actual } => {
            logger::error(
                "env",
                format!(
                    "{} 校验不通过（期望 {expected}，实际 {actual}），已放弃本次安装",
                    target.asset
                ),
            );
            // 不匹配 → 拒绝解压与写入，临时文件随守卫销毁（tasks 4.4）。
            Err(InstallError::Failed(format!(
                "{} 校验不通过，已放弃本次安装：期望 {}，实际 {}。{}",
                target.asset, expected, actual, MANUAL_PLACE_HINT
            )))
        }
        Verification::Unverified { reason } => {
            logger::warn(
                "env",
                format!(
                    "{} 未经校验（{reason}），等待用户确认后继续",
                    target.asset
                ),
            );
            // 未经校验 → 明确告知并要求确认（tasks 4.5）；先保留临时文件。
            let mut guard = guard;
            guard.keep();
            let pending = PendingInstall {
                staging,
                target,
                channel,
                channel_url: channel_url.clone(),
                archive: archive_path,
                unverified_reason: reason,
            };
            let outcome = pending.into_outcome();
            *state.pending.lock().unwrap() = Some(pending);
            Ok(outcome)
        }
        Verification::Verified { .. } => {
            logger::info(
                "env",
                format!("{} 校验通过（通道 {}）", target.asset, channel.label()),
            );
            let pending = PendingInstall {
                staging,
                target,
                channel,
                channel_url,
                archive: archive_path,
                unverified_reason: String::new(),
            };
            finish_install(app, pending).await
        }
    }
}

/// 解压 → 校验必需文件 → 原子替换（tasks 4.6 / 4.7）。
fn apply_pending(app: &AppHandle, pending: PendingInstall) -> Result<(), String> {
    let PendingInstall {
        staging,
        target,
        channel,
        archive,
        ..
    } = pending;
    let label = channel.label();
    // 成功或失败都要清掉临时目录（payload 会被改名移出，剩余残留一并删除）。
    let _guard = StagingGuard::new(staging.clone());

    let target_dir = scrcpy::scrcpy_dir(app)?;
    // 替换前再检一次（下载耗时较长，期间可能有会话启动）：先确认没有本应用会话在运行，
    // 再结束 adb 服务端，最后确认目录可替换（design D2）。占用状态取在释放之前，
    // 供替换阶段的进度文案说明这次多做了一件事。
    let had_occupier = occupied_file(&target_dir).is_some();
    prepare_replace(&session::active_ids(&app.state::<AppState>()), &target_dir)?;
    logger::info(
        "env",
        format!(
            "解压 {} 并替换运行环境目录 {}（v{}，通道 {label}）",
            target.asset,
            target_dir.display(),
            target.version
        ),
    );

    let raw = staging.join("raw");
    emit_progress(app, "extract", 0, format!("正在解压 {}…", target.asset), &label);
    let app_progress = app.clone();
    let label_progress = label.clone();
    // 解压方式随平台（zip / tar.gz）；tar.gz 的总条目数不可知，文案与百分比都退让一档。
    extract_archive(&archive, scrcpy::current()?, &raw, |done, total| {
        let percent = match total {
            Some(t) if t > 0 => (done * 100) / t,
            _ => 0,
        };
        let text = match total {
            Some(t) => format!("正在解压 {}（{} / {} 项）…", target.asset, done, t),
            None => format!("正在解压 {}（已解压 {} 项）…", target.asset, done),
        };
        emit_progress(&app_progress, "extract", percent as u8, text, &label_progress);
    })?;

    let payload = find_payload_root(&raw).ok_or_else(|| {
        format!(
            "解压结果中未找到 {}，已放弃本次安装（原运行环境未改动）。{}",
            scrcpy::SCRCPY_BIN,
            MANUAL_PLACE_HINT
        )
    })?;

    // 必需文件校验：解压结果必须自带 scrcpy 与 adb，否则不替换。
    let missing = assets::missing_bins(&payload);
    if !missing.is_empty() {
        return Err(format!(
            "解压结果缺少必需文件（{}），已放弃本次安装（原运行环境未改动）。{}",
            missing.join("、"),
            MANUAL_PLACE_HINT
        ));
    }

    emit_progress(
        app,
        "replace",
        100,
        if had_occupier {
            "正在结束 adb 服务端并替换运行环境目录…".to_string()
        } else {
            "正在替换运行环境目录…".to_string()
        },
        &label,
    );
    replace_dir(&payload, &target_dir)?;

    emit_progress(
        app,
        "done",
        100,
        format!("已安装 scrcpy v{}", target.version),
        &label,
    );
    Ok(())
}

/// 一次下载成功完成后清空本次运行内的通道记忆（design D7）。
///
/// 抽成独立函数：`finish_install` 需要 `AppHandle` 才能被测，而「成功后清空」这条语义
/// 本身既不需要网络也不需要窗口，单独拿出来就能断言。
fn forget_channels(state: &AppState) {
    state.channel_memory.lock().unwrap().clear();
}

/// 完成一次安装：先取出结论（`pending` 会被消费），再执行落盘。
///
/// 同步版本：解压与原子替换都是同步 FS + `zip` 解码，必须整段待在阻塞线程池上
/// （调用方负责，见 [`finish_install`] 与 `confirm_scrcpy_install`）。
fn finish_install_blocking(
    app: &AppHandle,
    pending: PendingInstall,
) -> Result<InstallOutcome, String> {
    let outcome = InstallOutcome {
        status: "done".to_string(),
        version: pending.target.version.clone(),
        asset: pending.target.asset.clone(),
        channel: pending.channel.label(),
        channel_url: pending.channel_url.clone(),
        verified: pending.unverified_reason.is_empty(),
        unverified_reason: if pending.unverified_reason.is_empty() {
            None
        } else {
            Some(pending.unverified_reason.clone())
        },
        note: pending.target.note.clone(),
    };
    apply_pending(app, pending)?;
    // 一次下载成功完成 ⇒ 清空通道记忆：下一次获取重新从官方通道开始尝试（design D7）。
    // 用户手动跳过的通道与它同寿命，一并清空（design D4）。
    // 「取消本次获取」走 `cancel_scrcpy_install` → 发信号 + `discard_pending`，不经过这里，
    // 因此那次没有成功完成的下载不会清空记忆。
    forget_channels(&app.state::<AppState>());
    Ok(outcome)
}

/// async 版本：把落盘整段放到阻塞线程池上，失败按 [`InstallError`] 归一。
async fn finish_install(
    app: &AppHandle,
    pending: PendingInstall,
) -> Result<InstallOutcome, InstallError> {
    let handle = app.clone();
    run_blocking(move || finish_install_blocking(&handle, pending))
        .await
        .map_err(InstallError::Failed)
}

// ── 检查更新（tasks 4.9） ──────────────────────────────────

/// 检查更新结果。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    /// 官方最新发布版本
    pub latest: String,
    /// 最佳适配版本
    pub best_fit: String,
    /// 已安装版本（探测失败为 `unknown`）
    pub installed: String,
    /// "upToDate" | "updateAvailable" | "installedNewer" | "notInstalled"
    pub state: String,
}

/// 比较「已安装版本」与「官方最新版本」得出结论（纯函数，覆盖 tasks 4.9 的两种结果）。
pub fn classify_update(installed: Option<&str>, latest: &str) -> (&'static str, String) {
    let installed = installed.unwrap_or(scrcpy::UNKNOWN_VERSION);
    match scrcpy::compare_versions(installed, latest) {
        Some(Ordering::Equal) => ("upToDate", installed.to_string()),
        Some(Ordering::Less) => ("updateAvailable", installed.to_string()),
        Some(Ordering::Greater) => ("installedNewer", installed.to_string()),
        // 未安装或版本不可解析：无从比较，由界面提示尚未获取运行环境。
        None => ("notInstalled", installed.to_string()),
    }
}

// ── Tauri 命令 ─────────────────────────────────────────────

/// 代理地址列表（design D9），供运行环境卡片的「下载通道」区域渲染。
///
/// `direct` 是首行的 GitHub 直连地址：只读、纯展示（不可点击、无跳转）。它**不在** `proxies` 里 ——
/// 直连不是代理，也没有可管理性（不可删、不可编辑），混进列表只会让前端不得不引入
/// 一个「是否只读」的标志再把它挑出来，而本次要删的正是那类标志。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyList {
    pub direct: String,
    pub proxies: Vec<String>,
}

fn proxy_list(state: &State<'_, AppState>) -> ProxyList {
    ProxyList {
        direct: RELEASES_PAGE.to_string(),
        proxies: proxy_addresses(state),
    }
}

#[tauri::command]
pub fn get_proxy_list(state: State<'_, AppState>) -> ProxyList {
    proxy_list(&state)
}

/// 追加一条代理地址：格式非法、重复项都拒绝（design D9）。
///
/// 抽成「只动配置、不落盘」的纯函数，是为了让「列表内一律可删、没有只读项」这条规则
/// 能被单测直接覆盖 —— 命令层只负责落盘与回传列表。
fn push_proxy(cfg: &mut config::AppConfig, raw: &str) -> Result<(), String> {
    let normalized = validate_proxy_address(raw)?;
    if cfg.channels.proxies.iter().any(|c| c == &normalized) {
        return Err("该地址已在列表中".to_string());
    }
    cfg.channels.proxies.push(normalized);
    Ok(())
}

/// 删除一条代理地址：**列表内一律可删**，包括出厂默认的那两条（design D9）。
fn remove_proxy(cfg: &mut config::AppConfig, url: &str) -> Result<(), String> {
    let before = cfg.channels.proxies.len();
    cfg.channels.proxies.retain(|c| c != url);
    if cfg.channels.proxies.len() == before {
        return Err("该地址不在列表中".to_string());
    }
    Ok(())
}

/// 追加代理地址：没有「内置项」这一类可对照 —— 出厂默认地址就在同一个列表里、
/// 走同一条路径（design D9）。
#[tauri::command]
pub fn add_proxy_address(
    app: AppHandle,
    state: State<'_, AppState>,
    url: String,
) -> Result<ProxyList, String> {
    {
        let mut cfg = state.config.lock().unwrap();
        push_proxy(&mut cfg, &url)?;
        let snapshot = cfg.clone();
        drop(cfg);
        config::save_config_to_disk(&app, &snapshot)?;
    }
    // 只改一处配置即可生效：通道列表每次下载时按配置现算。
    Ok(proxy_list(&state))
}

/// 删除代理地址（design D9）。
#[tauri::command]
pub fn remove_proxy_address(
    app: AppHandle,
    state: State<'_, AppState>,
    url: String,
) -> Result<ProxyList, String> {
    {
        let mut cfg = state.config.lock().unwrap();
        remove_proxy(&mut cfg, &url)?;
        let snapshot = cfg.clone();
        drop(cfg);
        config::save_config_to_disk(&app, &snapshot)?;
    }
    Ok(proxy_list(&state))
}

/// 检查更新（tasks 4.9）：查询官方最新发布版本并与已安装版本比较。
#[tauri::command]
pub async fn check_update(app: AppHandle, state: State<'_, AppState>) -> Result<UpdateInfo, String> {
    // 配置读取很快；网络请求是 async（design D1），不再需要阻塞线程池。
    let proxies = proxy_addresses(&state);
    let channels = resolve_channels(&proxies);
    let client = http_client()?;
    let state_ref = app.state::<AppState>();
    // 「检查更新」不是下载流程：**不发**尝试事件 —— 否则会在运行环境卡片上凭空拉起一条
    // 下载进度条（前端按 `installText` 非空展示该区块），且检查结束后无人清理它。
    // 但仍共用通道记忆：这里探明的不可达通道，后续下载同样跳过（design D2 / D5）。
    let (body, _) = attempt_channels(
        &state_ref.channel_memory,
        &channels,
        |_, _, _| {},
        |channel| {
            let url = channel.url_for(LATEST_RELEASE_API);
            let client = &client;
            async move { fetch_text(client, &url).await.map_err(AttemptError::Failure) }
        },
    )
    .await
    // 这条路径没有人工干预入口，`Canceled` 不会出现；真出现也只是把它当一条中止原因。
    .map_err(|abort| match abort {
        InstallAbort::AllFailed(reason) => reason,
        InstallAbort::Canceled => "已取消".to_string(),
    })?;
    let latest = parse_latest_release(&body)?.version;

    let dir = scrcpy::scrcpy_dir(&app)?;
    let installed = scrcpy::detect_installed_version(&dir);
    let (state_key, installed_text) = classify_update(installed.as_deref(), &latest);

    Ok(UpdateInfo {
        latest,
        best_fit: scrcpy::BEST_FIT_VERSION.to_string(),
        installed: installed_text,
        state: state_key.to_string(),
    })
}

/// 开始获取运行环境（design D1–D6）。
///
/// - `mode = "bestFit"`：首启缺失引导与「下载最佳适配版本」共用 —— 拉取构建期锁定的受支持版本；
/// - `mode = "latest"`：「下载最新版本」 —— 拉取官方最新发布版本（查询失败时会回退到 `bestFit`）。
///
/// 返回 `status = "needsConfirmation"` 时表示下载已完成但**无法获取官方校验值**，
/// 需前端向用户确认后调用 `confirm_scrcpy_install`；此时临时文件保留在应用数据根下。
/// 返回 `status = "canceled"` 时表示用户在下载/校验阶段点了「取消」——
/// 那不是错误，前端据此收起进度块而**不写** `installError`（design D6）。
#[tauri::command]
pub async fn start_scrcpy_install(
    app: AppHandle,
    mode: Option<String>,
) -> Result<InstallOutcome, String> {
    let latest = mode.as_deref().unwrap_or("bestFit") == "latest";
    let state = app.state::<AppState>();
    let target_dir = scrcpy::scrcpy_dir(&app)?;
    // ① 这里只确认没有本应用会话在运行（design D1）：有会话时替换会掐断它，必须先断开。
    //    目录被 adb 服务端占用**不在此拦截** —— 它由本应用的探测拉起，用户没有可断的东西；
    //    替换前会自动结束它（见 `prepare_replace`），此处只把这件事记进日志。
    ensure_sessions_stopped(&session::active_ids(&state))?;
    if occupied_file(&target_dir).is_some() {
        logger::info(
            "env",
            "运行环境目录内有进程占用（通常是本应用探测拉起的 adb 服务端），将在替换前结束它",
        );
    }
    // ② 同一时刻只允许一个在途安装：丢弃上一次的中间态。
    discard_pending(&state);
    // ③ 登记本次安装的控制入口：命令层拿得到它，才能把「切换通道 / 取消」送进在途循环
    //    （design D2）。每次安装换一份新的 ⇒ 不存在上一次留下的陈旧信号。
    let ctl = Arc::new(InstallCtl::new());
    *state.install.lock().unwrap() = Some(ctl.clone());

    let result = download_and_verify(&app, &state, latest, ctl).await;

    // 获取已结束（成功 / 失败 / 取消都算）：撤掉控制入口，界面上不该再残留可点击的入口。
    *state.install.lock().unwrap() = None;
    match result {
        Ok(outcome) => {
            if outcome.status == "done" {
                // 把「装成了哪个版本、走的哪条通道、有没有校验」一次记全。
                logger::info(
                    "env",
                    format!(
                        "运行环境安装完成：v{}（通道 {}，{}）",
                        outcome.version,
                        outcome.channel,
                        if outcome.verified {
                            "已通过官方校验"
                        } else {
                            "未经校验（用户已确认）"
                        }
                    ),
                );
            }
            Ok(outcome)
        }
        // 取消是**结论**而不是错误（design D6），日志里也不按错误记。
        Err(InstallError::Canceled) => {
            logger::info("env", "本次获取已被用户取消（原运行环境未改动）");
            Ok(InstallOutcome::canceled())
        }
        Err(InstallError::Failed(reason)) => {
            logger::error("env", format!("运行环境获取失败：{reason}"));
            Err(reason)
        }
    }
}

/// 「切换通道」：中断当前通道的尝试，改用下一条（design D6）。
///
/// 只在「当前没有进行中的获取」时报错 —— 按钮只在进行中出现，出现这个错误说明界面状态
/// 已经过期；提示一句比静默无反应更容易排查。
#[tauri::command]
pub fn switch_scrcpy_channel(state: State<'_, AppState>) -> Result<(), String> {
    match install_ctl(&state) {
        Some(ctl) => {
            logger::info("env", "用户请求切换下载通道");
            ctl.request_skip();
            Ok(())
        }
        None => Err("当前没有进行中的下载，无法切换通道".to_string()),
    }
}

/// 用户确认「未经校验继续安装」后继续（tasks 4.5）：复用已下载文件，不重复下载。
#[tauri::command]
pub async fn confirm_scrcpy_install(app: AppHandle) -> Result<InstallOutcome, String> {
    run_blocking(move || confirm_install_blocking(&app)).await
}

fn confirm_install_blocking(app: &AppHandle) -> Result<InstallOutcome, String> {
    let state = app.state::<AppState>();
    let pending = state
        .pending
        .lock()
        .unwrap()
        .take()
        .ok_or_else(|| "没有待确认的安装，请重新发起下载".to_string())?;
    match finish_install_blocking(app, pending) {
        Ok(outcome) => {
            logger::info(
                "env",
                format!(
                    "用户确认后继续安装完成：v{}（{}）",
                    outcome.version,
                    if outcome.verified {
                        "已通过官方校验"
                    } else {
                        "未经校验（用户已确认）"
                    }
                ),
            );
            Ok(outcome)
        }
        Err(e) => {
            logger::error("env", format!("运行环境安装失败：{e}"));
            Err(e)
        }
    }
}

/// 停止这次获取：中断在途的下载/校验，并丢弃待确认的中间态（design D6 / D8）。
///
/// 两件事都幂等 —— 没有在途安装、没有待确认态时是空操作。对用户而言「取消」就是同一件事，
/// 不该要求前端先判断自己处在哪个中间态才能决定调哪个命令。
#[tauri::command]
pub fn cancel_scrcpy_install(state: State<'_, AppState>) -> Result<(), String> {
    if let Some(ctl) = install_ctl(&state) {
        logger::info("env", "用户请求取消本次获取");
        ctl.request_cancel();
    }
    discard_pending(&state);
    Ok(())
}

// ── 人类可读体积（进度文案用） ─────────────────────────────

fn humansize(bytes: u64) -> String {
    const UNIT: f64 = 1024.0;
    let b = bytes as f64;
    if b < UNIT {
        return format!("{} B", bytes);
    }
    if b < UNIT * UNIT {
        return format!("{:.1} KB", b / UNIT);
    }
    format!("{:.1} MB", b / (UNIT * UNIT))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AppConfig;
    use zip::write::SimpleFileOptions;

    fn unique_temp_dir(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!("fcn-dl-{}-{}", tag, unique_suffix()))
    }

    fn write_zip(path: &Path, entries: &[(&str, &str)]) {
        let file = fs::File::create(path).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        let options = SimpleFileOptions::default();
        for (name, body) in entries {
            writer.start_file(*name, options).unwrap();
            writer.write_all(body.as_bytes()).unwrap();
        }
        writer.finish().unwrap();
    }

    /// 单测里的 async 执行器：复用 tauri 的全局运行时，不为测试再引一个 runtime 依赖。
    fn block_on<F: Future>(fut: F) -> F::Output {
        tauri::async_runtime::block_on(fut)
    }

    /// 「接受连接但永不回数据」的本地监听器：模拟一条僵死通道（design D1 的取消延迟对手）。
    ///
    /// 客户端视角下 `connect` 成功、请求也发得出去，只是永远等不到响应头 —— 正是
    /// `read_timeout` 才兜得住、而用户手动「切换 / 取消」应当立刻结束的那种状态。
    fn stall_listener() -> std::net::SocketAddr {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            // 收下第一条连接后就不再理会（也不关闭），第二条留在 backlog 里同样无人应答。
            let _held = listener.accept();
            std::thread::sleep(Duration::from_secs(20));
        });
        addr
    }

    /// 起一个在 `delay` 之后下达命令的线程，返回它持有的控制入口。
    fn ctl_with_delayed(cmd: CtlCmd, delay: Duration) -> Arc<InstallCtl> {
        let ctl = Arc::new(InstallCtl::new());
        let sender = Arc::clone(&ctl);
        std::thread::spawn(move || {
            std::thread::sleep(delay);
            match cmd {
                CtlCmd::SkipChannel => sender.request_skip(),
                CtlCmd::Cancel => sender.request_cancel(),
                CtlCmd::Run => {}
            }
        });
        ctl
    }

    // ── 通道与地址（design D9） ──

    #[test]
    fn default_proxies_are_seeded_by_the_config_model() {
        // 出厂默认地址的**唯一**声明点在 config.rs：download.rs 不再声明任何地址。
        assert_eq!(config::DEFAULT_PROXIES.len(), 2);
        assert!(config::DEFAULT_PROXIES.contains(&"https://gh-proxy.org"));
        assert!(config::DEFAULT_PROXIES.contains(&"https://ghproxy.net/"));
        // 出厂默认就是配置的初始内容，与用户追加项走同一条路径。
        assert_eq!(
            config::Channels::default().proxies,
            config::DEFAULT_PROXIES.to_vec()
        );
    }

    #[test]
    fn channel_order_is_official_then_the_configured_list() {
        let proxies = vec![
            "https://gh-proxy.org".to_string(),
            "https://ghproxy.net/".to_string(),
            "https://my-mirror.example.com".to_string(),
        ];
        let channels = resolve_channels(&proxies);

        assert_eq!(channels.len(), 4);
        assert_eq!(channels[0], Channel::Official);
        assert_eq!(
            channels[1],
            Channel::Proxy {
                url: "https://gh-proxy.org".to_string()
            }
        );
        assert_eq!(
            channels[3],
            Channel::Proxy {
                url: "https://my-mirror.example.com".to_string()
            }
        );
        // 没有「内置 / 自定义」的二分：所有代理一个标签，出厂默认也不例外。
        assert_eq!(channels[1].label(), "代理 https://gh-proxy.org");
        assert_eq!(channels[3].label(), "代理 https://my-mirror.example.com");
        assert_eq!(channels[0].label(), "GitHub 直连");
    }

    #[test]
    fn resolve_channels_drops_invalid_entries() {
        let proxies = vec![
            "ftp://nope".to_string(),
            "  ".to_string(),
            "https://ok.test".to_string(),
        ];
        let channels = resolve_channels(&proxies);
        // 官方 1 条 + 合法的 1 条；非法项被丢弃。
        assert_eq!(channels.len(), 2);
        assert_eq!(
            channels[1],
            Channel::Proxy {
                url: "https://ok.test".to_string()
            }
        );
    }

    #[test]
    fn proxy_url_construction_is_uniform() {
        let official = Channel::Official;
        let proxy = Channel::Proxy {
            url: "https://gh-proxy.org/".to_string(),
        };
        let origin = "https://github.com/Genymobile/scrcpy/releases/download/v5.0.1/x.zip";
        assert_eq!(official.url_for(origin), origin);
        assert_eq!(
            proxy.url_for(origin),
            "https://gh-proxy.org/https://github.com/Genymobile/scrcpy/releases/download/v5.0.1/x.zip"
        );
    }

    #[test]
    fn release_page_is_a_separate_address_from_the_asset_base() {
        // 首行只读项展示的是发布页，而资产地址由 `RELEASES_BASE` 拼出；两者不互相推导。
        assert_eq!(RELEASES_PAGE, "https://github.com/Genymobile/scrcpy/releases/");
        assert_ne!(RELEASES_PAGE.trim_end_matches('/'), RELEASES_BASE);
        assert!(RELEASES_BASE.starts_with(RELEASES_PAGE.trim_end_matches('/')));
    }

    #[test]
    fn validates_proxy_address_format() {
        assert_eq!(
            validate_proxy_address("  https://gh-proxy.org/ ").unwrap(),
            "https://gh-proxy.org"
        );
        assert_eq!(
            validate_proxy_address("http://192.168.1.10:8080").unwrap(),
            "http://192.168.1.10:8080"
        );
        assert!(validate_proxy_address("gh-proxy.org").is_err());
        assert!(validate_proxy_address("https://").is_err());
        assert!(validate_proxy_address("").is_err());
        assert!(validate_proxy_address("https://a b").is_err());
    }

    // ── 按序降级 ──

    #[test]
    fn degrades_to_next_channel_in_order() {
        let channels = resolve_channels(&[
            "https://gh-proxy.org".to_string(),
            "https://ghproxy.net/".to_string(),
        ]);
        let memory = Mutex::new(ChannelMemory::default());
        let tried: std::cell::RefCell<Vec<String>> = std::cell::RefCell::new(Vec::new());
        let tried_ref = &tried;

        let result = block_on(attempt_channels(
            &memory,
            &channels,
            no_attempt,
            |channel| {
                tried_ref.borrow_mut().push(channel.label());
                let failure = match channel {
                    Channel::Official => Some(AttemptFailure::body_reason(
                        "HTTP 403（可能被限流）".to_string(),
                    )),
                    Channel::Proxy { url } if url.contains("gh-proxy") => {
                        Some(AttemptFailure::body_reason("请求超时".to_string()))
                    }
                    _ => None,
                };
                let label = channel.label();
                async move {
                    match failure {
                        Some(failure) => Err(AttemptError::Failure(failure)),
                        None => Ok(label),
                    }
                }
            },
        ));

        let (label, channel) = result.unwrap();
        assert_eq!(channel, channels[2]);
        assert!(label.starts_with("代理 https://ghproxy.net"));
        assert_eq!(tried.borrow().len(), 3, "应在成功前依次尝试前两个通道");
    }

    // ── 全部失败 ──

    #[test]
    fn all_channels_failing_reports_each_reason_and_manual_hint() {
        let channels = resolve_channels(&[
            "https://gh-proxy.org".to_string(),
            "https://ghproxy.net/".to_string(),
            "https://mirror.test".to_string(),
        ]);
        let memory = Mutex::new(ChannelMemory::default());
        let err = block_on(attempt_channels(
            &memory,
            &channels,
            no_attempt,
            |channel| {
                let reason = format!("失败于 {}", channel.label());
                async move {
                    Err::<(), AttemptError>(AttemptError::Failure(AttemptFailure::body_reason(
                        reason,
                    )))
                }
            },
        ))
        .unwrap_err();

        let InstallAbort::AllFailed(err) = err else {
            panic!("全部通道失败时应报 AllFailed，而不是取消")
        };
        assert!(err.contains("GitHub 直连：失败于 GitHub 直连"));
        assert!(err.contains("gh-proxy.org"));
        assert!(err.contains("ghproxy.net"));
        assert!(err.contains("mirror.test"));
        assert!(err.contains(MANUAL_PLACE_HINT));
    }

    // ── 人工干预：切换 / 取消（design D2 / D4 / D6） ──

    #[test]
    fn switch_is_recorded_as_manual_skip_not_as_unreachable() {
        let channels = resolve_channels(&[
            "https://gh-proxy.org".to_string(),
            "https://ghproxy.net/".to_string(),
        ]);
        let memory = Mutex::new(ChannelMemory::default());

        let (_, chosen) = block_on(attempt_channels(
            &memory,
            &channels,
            no_attempt,
            |channel| {
                let switched = matches!(channel, Channel::Official);
                let label = channel.label();
                async move {
                    if switched {
                        Err(AttemptError::Switched)
                    } else {
                        Ok(label)
                    }
                }
            },
        ))
        .unwrap();

        assert_eq!(chosen, channels[1], "切换后应改用下一条通道");
        {
            let guard = memory.lock().unwrap();
            assert!(
                guard.manually_skipped(&channels[0]),
                "被用户跳过的通道应写入「手动跳过」，本次运行内不再尝试"
            );
            assert!(
                !guard.skipped(&channels[0]),
                "「太慢」不是「连不上」：不得写进不可达记忆（design D4）"
            );
        }

        // 本次运行内的下一次获取：被跳过的通道不再参与，序号按剩余通道计。
        let tried: std::cell::RefCell<Vec<String>> = std::cell::RefCell::new(Vec::new());
        let tried_ref = &tried;
        let (_, chosen) = block_on(attempt_channels(
            &memory,
            &channels,
            no_attempt,
            |channel| {
                tried_ref.borrow_mut().push(channel.label());
                let label = channel.label();
                async move { Ok::<_, AttemptError>(label) }
            },
        ))
        .unwrap();
        assert_eq!(chosen, channels[1]);
        assert_eq!(tried.borrow().len(), 1, "被跳过的通道不再被尝试");
        assert!(!tried.borrow()[0].contains("GitHub 直连"));
    }

    #[test]
    fn switched_channels_do_not_appear_in_the_failure_list() {
        let channels = resolve_channels(&["https://mirror.test".to_string()]);
        let memory = Mutex::new(ChannelMemory::default());
        let err = block_on(attempt_channels(
            &memory,
            &channels,
            no_attempt,
            |_channel| async { Err::<(), AttemptError>(AttemptError::Switched) },
        ))
        .unwrap_err();

        let InstallAbort::AllFailed(err) = err else {
            panic!("一路被跳过时应报一条可读的未完成，而不是取消")
        };
        assert!(
            err.contains("可用通道都被你跳过了"),
            "一路被跳过不得拼成一句空的原因列表，实际：{err}"
        );
    }

    #[test]
    fn cancel_aborts_the_whole_flow() {
        let channels = resolve_channels(&["https://mirror.test".to_string()]);
        let memory = Mutex::new(ChannelMemory::default());
        let err = block_on(attempt_channels(
            &memory,
            &channels,
            no_attempt,
            |_channel| async { Err::<(), AttemptError>(AttemptError::Canceled) },
        ))
        .unwrap_err();

        assert!(matches!(err, InstallAbort::Canceled));
        // 取消不是失败：不得被写成「无法建立连接」，也不该在原因列表里留下痕迹。
        let memory = memory.lock().unwrap();
        for channel in &channels {
            assert!(!memory.skipped(channel));
            assert!(!memory.manually_skipped(channel));
        }
    }

    #[test]
    fn canceled_outcome_is_a_status_not_an_error() {
        let outcome = InstallOutcome::canceled();
        assert_eq!(outcome.status, "canceled");
        assert!(outcome.version.is_empty() && outcome.channel.is_empty());
        assert!(outcome.note.is_none() && outcome.unverified_reason.is_none());

        // 前端按 camelCase 取 `status`；字段名即契约。
        let json = serde_json::to_value(&outcome).unwrap();
        assert_eq!(json["status"], "canceled");
        assert_eq!(json["channelUrl"], "");
    }

    #[test]
    fn stale_staging_dirs_are_cleaned_without_touching_others() {
        let root = unique_temp_dir("staging-clean");
        fs::create_dir_all(&root).unwrap();
        let stale = root.join(format!("{}111", STAGING_PREFIX));
        let keep = root.join(format!("scrcpy-win64-v{}", scrcpy::BEST_FIT_VERSION));
        fs::create_dir_all(&stale).unwrap();
        fs::create_dir_all(&keep).unwrap();
        fs::write(stale.join("part.zip"), b"partial").unwrap();

        cleanup_stale_staging(&root);

        assert!(!stale.exists());
        assert!(keep.exists());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn staging_guard_removes_dir_unless_kept() {
        let root = unique_temp_dir("guard");
        let kept = new_staging_dir(&root).unwrap();
        let dropped = new_staging_dir(&root).unwrap();

        let mut guard = StagingGuard::new(kept.clone());
        guard.keep();
        drop(guard);
        drop(StagingGuard::new(dropped.clone()));

        assert!(kept.exists(), "显式保留的临时目录不应被删除");
        assert!(!dropped.exists(), "失败路径必须不残留临时文件");
        let _ = fs::remove_dir_all(&root);
    }

    // ── 平台表项（两行的取值都要能被覆盖，故测试里显式取两个平台） ──

    fn win() -> &'static scrcpy::Platform {
        scrcpy::label_at("windows", "x86_64").unwrap()
    }

    fn linux() -> &'static scrcpy::Platform {
        scrcpy::label_at("linux", "x86_64").unwrap()
    }

    /// 本机平台的表项：需要与 `current()` 一致的断言用它，避免把 Windows 的取值写成通用值。
    fn here() -> &'static scrcpy::Platform {
        scrcpy::current().unwrap()
    }

    // ── 资产名（tasks 2.4） ──

    #[test]
    fn asset_at_covers_all_platforms() {
        // 四段都要对：`scrcpy-<平台标签>-v<版本><后缀>`。版本段取常量，升级时只改常量。
        let v = scrcpy::BEST_FIT_VERSION;
        assert_eq!(asset_at(win(), v), format!("scrcpy-win64-v{v}.zip"));
        assert_eq!(
            asset_at(scrcpy::label_at("windows", "aarch64").unwrap(), v),
            format!("scrcpy-winarm64-v{v}.zip")
        );
        assert_eq!(
            asset_at(linux(), v),
            format!("scrcpy-linux-x86_64-v{v}.tar.gz")
        );
    }

    #[test]
    fn best_fit_target_follows_platform() {
        let v = scrcpy::BEST_FIT_VERSION;
        let on_win = best_fit_target_at(win());
        assert_eq!(on_win.version, v);
        assert_eq!(on_win.asset, format!("scrcpy-win64-v{v}.zip"));
        assert_eq!(
            on_win.url,
            format!("{}/v{}/{}", RELEASES_BASE, v, on_win.asset)
        );
        assert!(on_win.note.is_none());

        let on_linux = best_fit_target_at(linux());
        assert_eq!(on_linux.asset, format!("scrcpy-linux-x86_64-v{v}.tar.gz"));
        assert!(on_linux.url.ends_with(&on_linux.asset));
    }

    // ── 4.4 校验 ──

    /// 官方 `SHA256SUMS.txt` 的夹具：三段（win64 zip、`*` 前缀写法、linux tar.gz）。
    /// 版本段取常量，升级时只改常量。
    fn sums() -> String {
        let v = scrcpy::BEST_FIT_VERSION;
        format!(
            "6b1a4a0d2f7c0b4c8b2f0a1d3e4f5a6b7c8d9e0f1a2b3c4d5e6f708192a3b4c5  scrcpy-win64-v{v}.zip\n\
             aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa *scrcpy-win64-v{v}.tar.gz\n\
             bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb  scrcpy-linux-x86_64-v{v}.tar.gz\n"
        )
    }

    #[test]
    fn parses_sha256sums_entry_by_asset_name() {
        assert_eq!(
            parse_sha256sums(&sums(), &asset_at(win(), scrcpy::BEST_FIT_VERSION)).as_deref(),
            Some("6b1a4a0d2f7c0b4c8b2f0a1d3e4f5a6b7c8d9e0f1a2b3c4d5e6f708192a3b4c5")
        );
        // 两平台的资产名都能在同一个汇总文件里定位到自己的条目。
        assert_eq!(
            parse_sha256sums(&sums(), &asset_at(linux(), scrcpy::BEST_FIT_VERSION)).as_deref(),
            Some("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb")
        );
    }

    #[test]
    fn parses_sha256sums_with_star_prefix() {
        let v = scrcpy::BEST_FIT_VERSION;
        let content = format!(
            "c0ffee0000000000000000000000000000000000000000000000000000000000 *scrcpy-win64-v{v}.zip\n"
        );
        assert_eq!(
            parse_sha256sums(&content, &format!("scrcpy-win64-v{v}.zip")).as_deref(),
            Some("c0ffee0000000000000000000000000000000000000000000000000000000000")
        );
    }

    #[test]
    fn sha256_file_matches_known_vector() {
        // `abc` 的 SHA-256（FIPS 180-4 示例向量）。
        let dir = unique_temp_dir("hash");
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join("abc.bin");
        fs::write(&file, b"abc").unwrap();

        let got = sha256_file(&file).unwrap();
        let _ = fs::remove_dir_all(&dir);

        assert_eq!(
            got,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn verification_covers_match_mismatch_and_missing_entry() {
        let asset = asset_at(win(), scrcpy::BEST_FIT_VERSION);
        let expected = "6b1a4a0d2f7c0b4c8b2f0a1d3e4f5a6b7c8d9e0f1a2b3c4d5e6f708192a3b4c5";

        assert_eq!(
            classify_verification(Ok(sums()), &asset, expected),
            Verification::Verified {
                expected: expected.to_string()
            }
        );

        match classify_verification(Ok(sums()), &asset, "deadbeef") {
            Verification::Mismatch { expected, actual } => {
                assert_eq!(expected, expected_of(&sums(), &asset));
                assert_eq!(actual, "deadbeef");
            }
            other => panic!("校验不通过时应判为 Mismatch，实际 {:?}", other),
        }

        match classify_verification(Ok("# no entries\n".to_string()), &asset, expected) {
            Verification::Unverified { reason } => assert!(reason.contains("缺少")),
            other => panic!("缺少条目时应判为未校验，实际 {:?}", other),
        }
    }

    fn expected_of(content: &str, asset: &str) -> String {
        parse_sha256sums(content, asset).unwrap()
    }

    // ── 4.5 无法获取官方校验值 ──

    #[test]
    fn sums_candidates_fall_back_and_report_all_failures() {
        // 首个候选（官方实际名）404 时改用无扩展名写法。
        let tried: std::cell::RefCell<Vec<String>> = std::cell::RefCell::new(Vec::new());
        let tried_ref = &tried;
        let got = block_on(fetch_sums_with(|name| {
            tried_ref.borrow_mut().push(name.to_string());
            let first = name == SUMS_CANDIDATES[0];
            async move {
                if first {
                    Err("HTTP 404（资产不存在）".to_string())
                } else {
                    Ok("content".to_string())
                }
            }
        }))
        .unwrap();
        assert_eq!(got, "content");
        assert_eq!(tried.borrow().clone(), SUMS_CANDIDATES.to_vec());

        let err = block_on(fetch_sums_with(|name| {
            let reason = format!("{} 不可达", name);
            async move { Err(reason) }
        }))
        .unwrap_err();
        assert!(err.contains("官方校验值文件不可获取"));
        assert!(err.contains(SUMS_CANDIDATES[0]));

        // 候选名与实际资产地址同目录（与 zip 同级）。
        let v = scrcpy::BEST_FIT_VERSION;
        let zip = format!(
            "https://github.com/Genymobile/scrcpy/releases/download/v{v}/scrcpy-win64-v{v}.zip"
        );
        assert_eq!(
            sibling_url(&zip, SUMS_ASSET).unwrap(),
            format!("https://github.com/Genymobile/scrcpy/releases/download/v{v}/{}", SUMS_ASSET)
        );
        assert_eq!(SUMS_ASSET, "SHA256SUMS.txt");
    }

    #[test]
    fn unverified_when_sums_fetch_failed() {
        let asset = asset_at(win(), scrcpy::BEST_FIT_VERSION);
        match classify_verification(Err("HTTP 403（可能被限流）".to_string()), &asset, "abc") {
            Verification::Unverified { reason } => {
                assert!(reason.contains("无法获取官方校验值"));
                assert!(reason.contains("403"));
            }
            other => panic!("拉取失败时应判为未校验，实际 {:?}", other),
        }
    }

    // ── 4.6 解压与原子替换 ──

    #[test]
    fn extracts_nested_release_layout_and_finds_payload() {
        // 与上面那条 tar.gz 同源：目录名与二进制名都按当前平台取，使这条用例在两个平台上都成立
        // —— `find_payload_root` 找的是**当前平台**的 `scrcpy` 可执行文件。
        let dir_name = scrcpy::dir_name_at(here(), scrcpy::BEST_FIT_VERSION);
        let files = [
            format!("{dir_name}/{}", scrcpy::SCRCPY_BIN),
            format!("{dir_name}/{}", assets::ADB_BIN),
            format!("{dir_name}/README.md"),
        ];

        let root = unique_temp_dir("extract");
        let raw = root.join("raw");
        let zip_path = root.join("asset.zip");
        fs::create_dir_all(&root).unwrap();
        write_zip(
            &zip_path,
            &[
                (files[0].as_str(), "SC"),
                (files[1].as_str(), "ADB"),
                (files[2].as_str(), "hi"),
            ],
        );

        extract_zip(&zip_path, &raw, |_, _| {}).unwrap();
        let payload = find_payload_root(&raw).unwrap();

        assert_eq!(payload, raw.join(&dir_name));
        assert!(assets::missing_bins(&payload).is_empty());
        let _ = fs::remove_dir_all(&root);
    }

    /// 造一个 `.tar.gz`（Linux 官方资产的格式）：条目名、内容与权限位都由调用方给定。
    fn write_tar_gz(path: &Path, entries: &[(&str, &str, u32)]) {
        let file = fs::File::create(path).unwrap();
        let encoder = flate2::write::GzEncoder::new(file, flate2::Compression::fast());
        let mut builder = tar::Builder::new(encoder);
        for (name, body, mode) in entries {
            let mut header = tar::Header::new_gnu();
            header.set_size(body.len() as u64);
            header.set_mode(*mode);
            header.set_cksum();
            builder.append_data(&mut header, name, body.as_bytes()).unwrap();
        }
        builder.into_inner().unwrap().finish().unwrap();
    }

    #[test]
    fn extracts_tar_gz_layout_and_finds_payload() {
        // 目录名与二进制名都按当前平台取，使这条用例在两个平台上都能跑：
        // 它验的是「多一层目录 + 深层路径都能铺开、payload 定位得到」，与具体标签无关。
        let dir_name = scrcpy::dir_name_at(here(), scrcpy::BEST_FIT_VERSION);
        let files = [
            format!("{dir_name}/{}", scrcpy::SCRCPY_BIN),
            format!("{dir_name}/{}", assets::ADB_BIN),
            format!("{dir_name}/nested/deep/README.md"),
        ];

        let root = unique_temp_dir("tar-extract");
        let raw = root.join("raw");
        let tarball = root.join("asset.tar.gz");
        fs::create_dir_all(&root).unwrap();
        write_tar_gz(
            &tarball,
            &[
                (files[0].as_str(), "SC", 0o755),
                (files[1].as_str(), "ADB", 0o755),
                (files[2].as_str(), "hi", 0o644),
            ],
        );

        // tar.gz 的总条目数不可知：回调的第二项恒为 None，条目数逐个递增。
        let mut reported: Vec<(usize, Option<usize>)> = Vec::new();
        extract_tar_gz(&tarball, &raw, |done, total| reported.push((done, total))).unwrap();

        assert_eq!(reported.len(), 3, "每个条目报一次");
        assert!(
            reported.iter().all(|(_, total)| total.is_none()),
            "tar.gz 的总数应为未知，实际：{reported:?}"
        );
        assert_eq!(
            reported.iter().map(|(done, _)| *done).collect::<Vec<_>>(),
            vec![1, 2, 3]
        );

        let payload = find_payload_root(&raw).unwrap();
        assert_eq!(payload, raw.join(&dir_name));
        assert!(assets::missing_bins(&payload).is_empty());
        // 深层目录照常铺开。
        assert!(raw.join(&files[2]).is_file());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    #[cfg(unix)]
    fn tar_gz_keeps_the_executable_bit() {
        // Linux 上「装上了但一跑就失败」的那道坎：权限位丢了就白装。
        // 只在类 Unix 上可断言（Windows 没有权限位这回事），故由 Linux 侧的 CI 覆盖。
        use std::os::unix::fs::PermissionsExt;

        let root = unique_temp_dir("tar-mode");
        let raw = root.join("raw");
        let tarball = root.join("asset.tar.gz");
        fs::create_dir_all(&root).unwrap();
        write_tar_gz(
            &tarball,
            &[
                ("scrcpy-linux-x86_64-v5.0.1/scrcpy", "SC", 0o755),
                ("scrcpy-linux-x86_64-v5.0.1/README.md", "hi", 0o644),
            ],
        );

        extract_tar_gz(&tarball, &raw, |_, _| {}).unwrap();

        let scrcpy_mode = fs::metadata(raw.join("scrcpy-linux-x86_64-v5.0.1/scrcpy"))
            .unwrap()
            .permissions()
            .mode();
        let readme_mode = fs::metadata(raw.join("scrcpy-linux-x86_64-v5.0.1/README.md"))
            .unwrap()
            .permissions()
            .mode();
        assert_ne!(scrcpy_mode & 0o111, 0, "scrcpy 必须保留可执行位");
        assert_eq!(readme_mode & 0o111, 0, "普通文件不应被赋予可执行位");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn escape_detection_covers_absolute_and_parent_entries() {
        // 「含 `..` 的条目被拒绝」这条防线直接测谓词：造不出这样的归档来端到端验
        // （`tar::Builder` 自己就拒绝写含 `..` 的条目：paths in archives must not have `..`）。
        // 端到端的第二道防线是 `unpack_in` 自身，它对越界条目也会拒绝。
        assert!(escapes_dest(Path::new("../escaped.txt")));
        assert!(escapes_dest(Path::new("scrcpy-linux-x86_64-v5.0.1/../../escaped.txt")));
        assert!(!escapes_dest(Path::new("scrcpy-linux-x86_64-v5.0.1/scrcpy")));
        assert!(!escapes_dest(Path::new("./a/b")));
        // 绝对路径同样越界（Windows 上盘符路径也算）。
        let absolute = if cfg!(windows) {
            Path::new(r"C:\Windows\System32\evil.dll")
        } else {
            Path::new("/etc/passwd")
        };
        assert!(escapes_dest(absolute));
    }

    #[test]
    fn extract_archive_dispatches_by_platform_suffix() {
        // 分派按平台后缀：同一份归档交给不对的平台只会得到一个明确的「不支持」。
        let root = unique_temp_dir("dispatch");
        let tarball = root.join("asset.tar.gz");
        fs::create_dir_all(&root).unwrap();
        write_tar_gz(&tarball, &[("scrcpy-linux-x86_64-v5.0.1/scrcpy", "SC", 0o755)]);

        let ok = root.join("ok");
        extract_archive(&tarball, linux(), &ok, |_, _| {}).unwrap();
        assert!(ok.join("scrcpy-linux-x86_64-v5.0.1/scrcpy").is_file());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn atomic_replace_keeps_old_env_when_zip_is_broken() {
        // 「解压中途失败」场景：原运行环境必须保持可用。
        let root = unique_temp_dir("replace-broken");
        let target = root.join("scrcpy-win64-v5.0.1");
        fs::create_dir_all(&target).unwrap();
        fs::write(target.join("scrcpy.exe"), b"OLD").unwrap();
        fs::write(target.join("adb.exe"), b"OLD").unwrap();

        let broken = root.join("broken.zip");
        fs::write(&broken, b"this is not a zip").unwrap();
        let raw = root.join("raw");

        let err = extract_zip(&broken, &raw, |_, _| {}).unwrap_err();
        assert!(err.contains("无法解析"));

        // 目标目录原样保留（未被触碰）。
        assert_eq!(fs::read(target.join("scrcpy.exe")).unwrap(), b"OLD");
        assert_eq!(fs::read(target.join("adb.exe")).unwrap(), b"OLD");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn atomic_replace_moves_payload_into_place_and_drops_backup() {
        let root = unique_temp_dir("replace-ok");
        let target = root.join("scrcpy-win64-v5.0.1");
        let payload = root.join("payload");
        fs::create_dir_all(&target).unwrap();
        fs::create_dir_all(&payload).unwrap();
        // 二进制名按当前平台取：`missing_bins` 查的是**本平台**的必需文件，写死 `.exe` 只在 Windows 上成立。
        fs::write(target.join(scrcpy::SCRCPY_BIN), b"OLD").unwrap();
        fs::write(payload.join(scrcpy::SCRCPY_BIN), b"NEW").unwrap();
        fs::write(payload.join(assets::ADB_BIN), b"NEW").unwrap();

        replace_dir(&payload, &target).unwrap();

        assert_eq!(fs::read(target.join(scrcpy::SCRCPY_BIN)).unwrap(), b"NEW");
        assert!(assets::missing_bins(&target).is_empty());
        // 备份目录已被清理。
        let leftovers: Vec<_> = fs::read_dir(&root)
            .unwrap()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().contains("backup"))
            .collect();
        assert!(leftovers.is_empty());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn payload_without_required_bins_is_rejected() {
        // 只放 `scrcpy`、不放 `adb`：夹具的名字按当前平台取，否则 Linux 上 `find_payload_root`
        // 连 payload 都定位不到，测的就不是「缺 adb 被拒」而是别的东西。
        let dir_name = scrcpy::dir_name_at(here(), scrcpy::BEST_FIT_VERSION);
        let only_scrcpy = format!("{dir_name}/{}", scrcpy::SCRCPY_BIN);

        let root = unique_temp_dir("payload-partial");
        let raw = root.join("raw");
        let zip_path = root.join("asset.zip");
        fs::create_dir_all(&root).unwrap();
        write_zip(&zip_path, &[(only_scrcpy.as_str(), "SC")]);

        extract_zip(&zip_path, &raw, |_, _| {}).unwrap();
        let payload = find_payload_root(&raw).unwrap();

        assert_eq!(assets::missing_bins(&payload), vec![assets::ADB_BIN.to_string()]);
        let _ = fs::remove_dir_all(&root);
    }

    // ── 占用检测与释放（design D2 / D6） ──

    #[test]
    fn reports_the_occupied_file_and_clears_after_release() {
        let dir = unique_temp_dir("in-use");
        fs::create_dir_all(&dir).unwrap();
        let exe = dir.join(scrcpy::SCRCPY_BIN);
        fs::write(&exe, b"stub").unwrap();

        assert_eq!(occupied_file(&dir), None, "无占用时不得误报");

        // 独占打开（不允许共享）模拟「正在运行的可执行映像」持有的句柄。
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            let held = fs::OpenOptions::new()
                .read(true)
                .share_mode(0)
                .open(&exe)
                .unwrap();
            assert_eq!(
                occupied_file(&dir),
                Some(exe.clone()),
                "报出的应是被占用的那个文件本身"
            );
            drop(held);
        }

        assert_eq!(occupied_file(&dir), None);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn ensure_dir_free_names_the_occupied_file() {
        let dir = unique_temp_dir("ensure-free");
        fs::create_dir_all(&dir).unwrap();

        // 目录不存在 / 未被占用都放行。
        assert!(ensure_dir_free(&dir).is_ok());
        assert!(ensure_dir_free(&dir.join("absent")).is_ok());

        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            // 用 adb.exe 举例：真实缺陷里被占用的正是这个文件（探测拉起的服务端）。
            let adb = dir.join(assets::ADB_BIN);
            fs::write(&adb, b"stub").unwrap();
            let held = fs::OpenOptions::new()
                .read(true)
                .share_mode(0)
                .open(&adb)
                .unwrap();
            let err = ensure_dir_free(&dir).unwrap_err();
            assert!(err.contains(assets::ADB_BIN), "必须点名被占用的文件：{err}");
            // 没有会话在运行时不得声称「可能有 scrcpy 会话未断开」—— 那正是原来的缺陷。
            assert!(!err.contains("会话"), "不得把占用一律说成会话未断开：{err}");
            drop(held);
        }
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn sessions_running_blocks_the_replace_with_an_accurate_hint() {
        let mut active = HashSet::new();
        assert!(ensure_sessions_stopped(&active).is_ok());

        active.insert("dev-1".to_string());
        let err = ensure_sessions_stopped(&active).unwrap_err();
        assert!(
            err.contains("请先断开连接"),
            "有会话时提示应指向「断开连接」：{err}"
        );
        assert!(err.contains('1'), "提示应给出在运行的会话数：{err}");
    }

    #[test]
    fn release_is_gated_and_requires_the_binary() {
        // 平台分流：本平台不因占用而拒绝替换时，也没有理由去干扰共享的 adb 服务端。
        let dir = unique_temp_dir("release-gate");
        fs::create_dir_all(&dir).unwrap();
        match release_adb_server(&dir) {
            // 类 Unix：直接返回「没做」。
            Ok(released) => {
                assert!(!occupancy_blocks_replace());
                assert!(!released, "类 Unix 上不应发起该调用");
            }
            // Windows：目录内没有 adb，结束动作应报出缺少的那个文件。
            Err(e) => {
                assert!(occupancy_blocks_replace());
                assert!(e.contains(assets::ADB_BIN), "应点名缺少的二进制：{e}");
            }
        }
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    #[cfg(windows)]
    fn release_reports_success_nonzero_and_timeout() {
        use std::time::Instant;

        // 三种返回都要能区分：成功放行，非零退出与超时都报错（后者由占用检查兜底）。
        // 用系统命令当桩：真实 adb 只在用户机器上存在。
        // 必须给绝对路径 —— 辅助函数先查 `is_file()`，相对路径会相对当前目录解析。
        let cmd = std::env::var_os("COMSPEC")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(r"C:\Windows\System32\cmd.exe"));
        assert!(cmd.is_file(), "取不到系统命令解释器：{}", cmd.display());
        assert!(run_adb_kill_server(&cmd, &["/C", "exit", "0"], Duration::from_secs(10)).is_ok());

        let err =
            run_adb_kill_server(&cmd, &["/C", "exit", "1"], Duration::from_secs(10)).unwrap_err();
        assert!(!err.is_empty(), "非零退出必须给出失败原因");

        let started = Instant::now();
        let err = run_adb_kill_server(
            &cmd,
            &["/C", "ping", "-n", "10", "127.0.0.1"],
            Duration::from_millis(300),
        )
        .unwrap_err();
        assert!(err.contains("超时"), "实际错误：{err}");
        assert!(started.elapsed() < Duration::from_secs(8), "超时后应立即返回");

        // 缺二进制时报出文件名，而不是静默成功。
        let err = run_adb_kill_server(
            Path::new(r"C:\definitely\not\here\adb.exe"),
            &["kill-server"],
            Duration::from_secs(1),
        )
        .unwrap_err();
        assert!(err.contains("adb.exe"));
    }

    #[test]
    fn prepare_replace_covers_release_and_still_blocked_paths() {
        let dir = unique_temp_dir("prepare-replace");
        fs::create_dir_all(&dir).unwrap();
        let adb = dir.join(assets::ADB_BIN);
        fs::write(&adb, b"stub").unwrap();
        let no_sessions = HashSet::new();

        // ① 无会话 + 目录未被占用：即便「结束服务端」这一步失败（stub 不是有效可执行文件），
        //    也不该把安装打断在释放失败上 —— 放行，由替换本身继续。
        assert!(prepare_replace(&no_sessions, &dir).is_ok());

        // ② 有会话：拒绝，提示指向「断开连接」。
        let mut active = HashSet::new();
        active.insert("dev-1".to_string());
        let err = prepare_replace(&active, &dir).unwrap_err();
        assert!(err.contains("请先断开连接"));

        // ③ 无会话但文件仍被占用：不替换，并点名那个文件。
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            let held = fs::OpenOptions::new()
                .read(true)
                .share_mode(0)
                .open(&adb)
                .unwrap();
            let err = prepare_replace(&no_sessions, &dir).unwrap_err();
            assert!(err.contains(assets::ADB_BIN), "必须点名被占用的文件：{err}");
            drop(held);
        }
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn occupancy_gate_follows_platform() {
        // tasks 2.5：本平台是否会因「目录被占用」而拒绝替换。
        // Windows 必须拒绝（运行中的映像被系统持有句柄，替换会失败在半途）；
        // 类 Unix 不需要这份检测 —— 允许改名目录、允许 unlink 已打开的文件，
        // 替换对运行中的会话无害（理由见 `occupancy_blocks_replace`）。
        assert_eq!(occupancy_blocks_replace(), cfg!(windows));

        // 未被占用的目录两平台都放行。
        let dir = unique_temp_dir("occupancy");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(scrcpy::SCRCPY_BIN), b"stub").unwrap();
        assert!(ensure_dir_free(&dir).is_ok());
        let _ = fs::remove_dir_all(&dir);
    }

    // ── 4.9 检查更新 ──

    #[test]
    fn classifies_update_results() {
        let v = scrcpy::BEST_FIT_VERSION;
        assert_eq!(classify_update(Some(v), v).0, "upToDate");
        assert_eq!(classify_update(Some("4.0"), v).0, "updateAvailable");
        assert_eq!(classify_update(Some("6.0"), v).0, "installedNewer");
        assert_eq!(classify_update(None, v).0, "notInstalled");
        assert_eq!(classify_update(None, v).1, scrcpy::UNKNOWN_VERSION);
    }

    #[test]
    fn parses_latest_release_json() {
        // 资产名按当前平台拼：这条断言必须在两个平台上都成立（否则它只是 Windows 的断言）。
        let asset = asset_at(here(), "4.2");
        let body = format!(
            r#"{{"tag_name":"v4.2","assets":[{{"name":"{asset}","browser_download_url":"https://example.com/a"}},{{"name":"SHA256SUMS","browser_download_url":"https://example.com/SHA256SUMS"}}]}}"#
        );
        let target = parse_latest_release(&body).unwrap();
        assert_eq!(target.version, "4.2");
        assert_eq!(target.asset, asset);
        assert_eq!(target.url, "https://example.com/a");
        assert_eq!(
            sibling_url(&target.url, SUMS_ASSET).unwrap(),
            format!("https://example.com/{}", SUMS_ASSET)
        );
    }

    #[test]
    fn release_asset_without_url_falls_back_to_conventional_address() {
        // 条目在、只是没给下载地址：仍按本平台的约定拼地址（拼出来的仍是本平台的包）。
        let asset = asset_at(here(), "4.2");
        let body = format!(r#"{{"tag_name":"v4.2","assets":[{{"name":"{asset}"}}]}}"#);
        let target = parse_latest_release(&body).unwrap();
        assert_eq!(target.asset, asset);
        assert_eq!(target.url, format!("{}/v4.2/{}", RELEASES_BASE, asset));
    }

    #[test]
    fn release_without_this_platforms_asset_is_an_error() {
        // spec「按当前平台匹配发布资产」：官方没有本平台的包时明确报错，MUST NOT 回退。
        let body = r#"{"tag_name": "v4.2", "assets": []}"#;
        let err = parse_latest_release(body).unwrap_err();
        assert!(err.contains("没有本平台的发布包"), "实际：{err}");
        assert!(
            err.contains(&asset_at(here(), "4.2")),
            "错误信息应给出本平台该有的资产名，实际：{err}"
        );
    }

    #[test]
    fn release_with_only_another_platforms_asset_is_an_error() {
        // 列表里有别的平台的资产、本平台那份缺失 —— 同样报错，不拿别的平台的包顶上。
        let other = if cfg!(windows) {
            asset_at(linux(), "4.2")
        } else {
            asset_at(win(), "4.2")
        };
        let body = format!(
            r#"{{"tag_name":"v4.2","assets":[{{"name":"{other}","browser_download_url":"https://example.invalid/x"}}]}}"#
        );
        let err = parse_latest_release(&body).unwrap_err();
        assert!(err.contains("没有本平台的发布包"), "实际：{err}");
    }

    #[test]
    fn status_code_descriptions_mention_rate_limit() {
        assert!(describe_status(reqwest::StatusCode::FORBIDDEN).contains("限流"));
        assert!(describe_status(reqwest::StatusCode::TOO_MANY_REQUESTS).contains("限流"));
        assert_eq!(describe_status(reqwest::StatusCode::NOT_FOUND), "HTTP 404（资产不存在）");
    }

    // ── 通道预算与停滞预算（design D3） ──

    #[test]
    fn channel_attempt_budgets_are_ordered() {
        assert!(
            CHANNEL_CONNECT_TIMEOUT >= Duration::from_secs(3),
            "连接预算过小会在慢网络上把可用通道误判为不可达"
        );
        assert!(
            CHANNEL_IDLE_TIMEOUT > Duration::from_secs(3),
            "停滞预算要有余量：正常涓流下载的数据也是间歇到达的"
        );
        assert!(
            CHANNEL_CONNECT_TIMEOUT < CHANNEL_IDLE_TIMEOUT,
            "连接与等响应头共用停滞预算：连接预算应更紧，黑洞通道才会先死"
        );
        assert!(
            CHANNEL_IDLE_TIMEOUT < CHANNEL_TOTAL_TIMEOUT,
            "停滞预算必须显著短于整体预算，否则「僵死即降级」无从发生"
        );
        assert!(http_client().is_ok(), "预算值本身也要能构造出客户端");
    }

    // ── 人工干预的即时性（design D1 / D2） ──

    /// 控制信号必须能打断**在途请求**，而不是等本次读超时（design D1）。
    ///
    /// 对手是一条僵死通道：`send()` 之后永远等不到响应头。若没有 `race_ctl` 包住 `send()`，
    /// 这条用例会一直挂到 `CHANNEL_IDLE_TIMEOUT`（10s）才返回 —— 断言「远小于它」锁住即时性。
    #[test]
    fn skip_interrupts_an_in_flight_request() {
        let addr = stall_listener();
        let client = http_client().unwrap();
        let ctl = ctl_with_delayed(CtlCmd::SkipChannel, Duration::from_millis(150));
        let dest = std::env::temp_dir().join(format!("fcn-skip-{}.zip", unique_suffix()));
        let url = format!("http://{}/x.zip", addr);

        let started = std::time::Instant::now();
        let outcome = block_on(download_to_file(&client, &url, &dest, &ctl, |_, _| {}));
        let elapsed = started.elapsed();
        let _ = fs::remove_file(&dest);

        assert!(
            matches!(outcome, Err(AttemptError::Switched)),
            "「切换通道」应让本次尝试以 Switched 结束，实际 {outcome:?}"
        );
        assert!(
            elapsed < CHANNEL_IDLE_TIMEOUT / 2,
            "切换必须即时生效，不该等停滞预算兜底，实际 {elapsed:?}"
        );
    }

    #[test]
    fn cancel_interrupts_an_in_flight_request() {
        let addr = stall_listener();
        let client = http_client().unwrap();
        let ctl = ctl_with_delayed(CtlCmd::Cancel, Duration::from_millis(150));
        let dest = std::env::temp_dir().join(format!("fcn-cancel-{}.zip", unique_suffix()));
        let url = format!("http://{}/x.zip", addr);

        let started = std::time::Instant::now();
        let outcome = block_on(download_to_file(&client, &url, &dest, &ctl, |_, _| {}));
        let elapsed = started.elapsed();
        let _ = fs::remove_file(&dest);

        assert!(matches!(outcome, Err(AttemptError::Canceled)));
        assert!(
            elapsed < CHANNEL_IDLE_TIMEOUT / 2,
            "取消必须即时生效，实际 {elapsed:?}"
        );
    }

    /// 同一次安装内的**第二次**切换必须同样生效（design D2）。
    ///
    /// 这是「命令字读取即复位」的存在理由：若照「值变了才通知」去做（`watch` 的语义），
    /// 第二次点「切换通道」会因为值仍是 `SkipChannel` 而被静默丢掉。
    #[test]
    fn a_second_switch_in_the_same_install_is_not_swallowed() {
        let addr = stall_listener();
        let client = http_client().unwrap();
        let ctl = Arc::new(InstallCtl::new());
        let dest = std::env::temp_dir().join(format!("fcn-twice-{}.zip", unique_suffix()));
        let url = format!("http://{}/x.zip", addr);

        // 第一次：命令先到、请求随后 —— 循环首轮即取走并复位。
        ctl.request_skip();
        let first = block_on(download_to_file(&client, &url, &dest, &ctl, |_, _| {}));
        assert!(matches!(first, Err(AttemptError::Switched)));

        // 第二次：命令在请求在途时到达 —— 必须照样生效。
        let skipper = Arc::clone(&ctl);
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(150));
            skipper.request_skip();
        });
        let started = std::time::Instant::now();
        let second = block_on(download_to_file(&client, &url, &dest, &ctl, |_, _| {}));
        let elapsed = started.elapsed();
        let _ = fs::remove_file(&dest);

        assert!(
            matches!(second, Err(AttemptError::Switched)),
            "第二次切换被吞掉了，实际 {second:?}"
        );
        assert!(elapsed < CHANNEL_IDLE_TIMEOUT / 2, "实际 {elapsed:?}");
    }

    /// 取校验值阶段只认取消：切换请求就地丢弃，不泄漏到后面的阶段（design D5）。
    #[test]
    fn skip_is_dropped_during_the_verify_phase() {
        let ctl = InstallCtl::new();
        ctl.request_skip();

        // `race_cancel` 收到切换请求后应当继续等 future，而不是把它当成中止。
        let outcome = block_on(race_cancel(&ctl, async { Ok::<u8, String>(7) }));
        assert!(matches!(outcome, Ok(Ok(7))));
        // 已被取走 ⇒ 不会留给后面的下载阶段。
        assert_eq!(ctl.take_if_any(), None);

        // 取消则照常中止。
        ctl.request_cancel();
        let canceled = block_on(race_cancel(
            &ctl,
            std::future::pending::<Result<u8, String>>(),
        ));
        assert!(matches!(canceled, Err(AttemptError::Canceled)));
    }

    // ── 通道记忆与尝试可见性（download-channel-failover） ──

    /// 不需要「尝试通报」的调用点（`check_update` 与多数单测）。
    fn no_attempt(_: u32, _: u32, _: &Channel) {}

    #[test]
    fn clear_forgets_both_kinds_of_memory() {
        let proxy = Channel::Proxy {
            url: "https://gh-proxy.org".to_string(),
        };
        let mut memory = ChannelMemory::default();
        memory.mark_unreachable(&Channel::Official);
        memory.mark_unreachable(&Channel::Official); // 去重
        memory.mark_skipped(&proxy);
        memory.mark_skipped(&proxy); // 去重
        assert!(memory.skipped(&Channel::Official));
        assert!(memory.manually_skipped(&proxy));
        // 两类互不串台：「太慢」不等于「连不上」。
        assert!(!memory.manually_skipped(&Channel::Official));
        assert!(!memory.skipped(&proxy));

        memory.clear();
        assert!(
            !memory.skipped(&Channel::Official) && !memory.manually_skipped(&proxy),
            "清空 ⇒ 两类记忆一并失效，下一次获取重新从官方通道开始（design D4 / D7）"
        );
    }

    #[test]
    fn forget_channels_clears_the_state_memory() {
        let state = AppState::default();
        let proxy = Channel::Proxy {
            url: "https://gh-proxy.org".to_string(),
        };
        {
            let mut memory = state.channel_memory.lock().unwrap();
            memory.mark_unreachable(&Channel::Official);
            memory.mark_skipped(&proxy);
        }

        forget_channels(&state);

        let memory = state.channel_memory.lock().unwrap();
        assert!(!memory.skipped(&Channel::Official));
        assert!(
            !memory.manually_skipped(&proxy),
            "成功安装后手动跳过与不可达记忆同寿命，一并清空（design D4）"
        );
    }

    #[test]
    fn only_send_stage_connection_failures_are_unreachable() {
        // 请求发出阶段的连接类失败 ⇒ 通道不可达（生产路径：`send_failure`）
        let connect = classify_failure(FailureStage::Send, true, false, "无法建立连接".to_string());
        assert!(connect.unreachable, "连接阶段连不上 ⇒ 通道不可达");

        let timeout = classify_failure(FailureStage::Send, false, true, "请求超时".to_string());
        assert!(timeout.unreachable, "连接阶段超时 ⇒ 通道不可达");

        // HTTP 状态码（限流 / 资产不存在）：手上没有 `reqwest::Error`，走 reason 入口
        let status = AttemptFailure::body_reason(describe_status(reqwest::StatusCode::FORBIDDEN));
        assert!(!status.unreachable, "限流是瞬时的，官方 403 不得让通道被跳过");
        assert!(status.reason.contains("限流"));

        // 已连上之后的读体中断：即便带着连接 / 超时标志，阶段也必须把它挡掉
        // （`body_failure` 正是把真实标志原样传进来的那个表达式）
        let body = classify_failure(FailureStage::Body, true, true, "下载中断".to_string());
        assert!(
            !body.unreachable,
            "读到过响应体即证明链路是通的 ⇒ 传输中断不算通道不可达"
        );
    }

    #[test]
    fn usable_channels_skips_both_kinds_and_never_returns_empty() {
        let all = resolve_channels(&[
            "https://gh-proxy.org".to_string(),
            "https://ghproxy.net/".to_string(),
        ]);
        assert_eq!(
            usable_channels(&ChannelMemory::default(), &all),
            all,
            "记忆为空时不得改变顺序"
        );

        let mut memory = ChannelMemory::default();
        memory.mark_unreachable(&Channel::Official);
        assert_eq!(
            usable_channels(&memory, &all),
            all[1..].to_vec(),
            "不可达的通道应被移除，其余保持原顺序"
        );

        // 用户手动跳过的通道同样不再参与（design D4）。
        memory.mark_skipped(&all[1]);
        assert_eq!(
            usable_channels(&memory, &all),
            all[2..].to_vec(),
            "手动跳过的通道也被滤除"
        );

        for channel in &all {
            memory.mark_unreachable(channel);
        }
        assert_eq!(
            usable_channels(&memory, &all),
            all,
            "全部被跳过时必须回退为「不过滤」，否则会报出一条根本没试过任何通道的假失败"
        );
    }

    #[test]
    fn attempt_payload_carries_index_without_claiming_a_channel() {
        let progress = InstallProgress::attempt(
            2,
            3,
            &Channel::Proxy {
                url: "https://gh-proxy.org".to_string(),
            },
        );
        assert_eq!((progress.attempt, progress.attempt_total), (2, 3));
        assert_eq!(progress.phase, "download");
        assert_eq!(progress.attempt_channel, "代理 https://gh-proxy.org");
        assert!(
            progress.channel.is_empty(),
            "尚未生效的通道不得写进 channel —— 它表示的是「已生效的通道」"
        );
        assert!(
            progress.text.is_empty(),
            "尝试事件不带文案：这句话（含已耗时秒数）由前端拼，后端再拼一遍就会与它并排重复"
        );

        let plain = InstallProgress::plain("extract", 50, "正在解压".to_string(), "GitHub 直连");
        assert_eq!((plain.attempt, plain.attempt_total), (0, 0));
        assert!(plain.attempt_channel.is_empty(), "普通进度事件不带尝试信息");

        // 前端按 camelCase 取值，字段名即契约。
        let json = serde_json::to_value(&progress).unwrap();
        assert_eq!(json["attempt"], 2);
        assert_eq!(json["attemptTotal"], 3);
        assert_eq!(json["attemptChannel"], progress.attempt_channel);
        assert_eq!(json["channel"], "");
    }

    #[test]
    fn attempts_are_announced_before_they_run() {
        let channels = resolve_channels(&[
            "https://gh-proxy.org".to_string(),
            "https://ghproxy.net/".to_string(),
        ]);
        let memory = Mutex::new(ChannelMemory::default());
        let log: std::cell::RefCell<Vec<String>> = std::cell::RefCell::new(Vec::new());
        let log_ref = &log;

        let (_, chosen) = block_on(attempt_channels(
            &memory,
            &channels,
            |index, total, channel| {
                log_ref
                    .borrow_mut()
                    .push(format!("尝试 {index}/{total} {}", channel.label()));
            },
            |channel| {
                log_ref.borrow_mut().push(format!("请求 {}", channel.label()));
                let failure = match channel {
                    // 官方连不上（连接阶段）⇒ 应被记入记忆
                    Channel::Official => Some(classify_failure(
                        FailureStage::Send,
                        true,
                        false,
                        "无法建立连接".to_string(),
                    )),
                    // 第一条代理被限流 ⇒ 不得被记入记忆
                    Channel::Proxy { url } if url.contains("gh-proxy") => Some(
                        AttemptFailure::body_reason("HTTP 403（可能被限流）".to_string()),
                    ),
                    _ => None,
                };
                let label = channel.label();
                async move {
                    match failure {
                        Some(failure) => Err(AttemptError::Failure(failure)),
                        None => Ok(label),
                    }
                }
            },
        ))
        .unwrap();

        assert!(chosen.label().starts_with("代理 https://ghproxy.net"));
        let log = log.into_inner();
        assert_eq!(log.len(), 6, "每次尝试都应「先通报、后发请求」");
        for (i, channel) in channels.iter().enumerate() {
            assert_eq!(log[i * 2], format!("尝试 {}/3 {}", i + 1, channel.label()));
            assert_eq!(log[i * 2 + 1], format!("请求 {}", channel.label()));
        }

        assert!(
            memory.lock().unwrap().skipped(&channels[0]),
            "连接类失败应被记住"
        );
        assert!(
            !memory.lock().unwrap().skipped(&channels[1]),
            "限流类失败不得进入记忆"
        );

        // 第二轮：官方被跳过，不再重复付一次连接等待。
        let second: std::cell::RefCell<Vec<String>> = std::cell::RefCell::new(Vec::new());
        let second_ref = &second;
        let (_, chosen) = block_on(attempt_channels(
            &memory,
            &channels,
            |index, total, channel| {
                second_ref
                    .borrow_mut()
                    .push(format!("尝试 {index}/{total} {}", channel.label()));
            },
            |channel| {
                let label = channel.label();
                async move { Ok::<_, AttemptError>(label) }
            },
        ))
        .unwrap();
        let second = second.into_inner();
        assert_eq!(second.len(), 1, "首条可用通道即成功，只应有一次尝试");
        assert_eq!(
            second[0],
            format!("尝试 1/2 {}", channels[1].label()),
            "总数由 3 变 2：被记住的通道已不参与，序号按「本次可用通道」计"
        );
        assert!(
            !second.iter().any(|entry| entry.contains("GitHub 直连")),
            "被记住的通道必须被跳过"
        );
        assert_eq!(chosen, channels[1]);
    }

    #[test]
    fn best_fit_target_carries_no_discovered_channel() {
        let channels = resolve_channels(&[
            "https://gh-proxy.org".to_string(),
            "https://ghproxy.net/".to_string(),
        ]);
        let memory = Mutex::new(ChannelMemory::default());
        let ctl = InstallCtl::new();
        // 只构造客户端，不发起任何请求。
        let client = http_client().unwrap();

        let (target, discovered) =
            block_on(resolve_target(&channels, &client, false, &memory, &ctl, no_attempt)).unwrap();

        assert_eq!(target.version, scrcpy::BEST_FIT_VERSION);
        assert!(
            discovered.is_none(),
            "bestFit 不查询 API，因而没有「本流程内已确认可用」的通道"
        );
    }

    #[test]
    fn api_result_selects_its_channel_and_failure_falls_back_with_a_note() {
        let proxy = Channel::Proxy {
            url: "https://gh-proxy.org".to_string(),
        };
        // 资产名按当前平台拼：这条断言（与整条用例）在两个平台上都要能跑。
        let asset = asset_at(here(), "4.2");
        let body = format!(
            r#"{{"tag_name":"v4.2","assets":[{{"name":"{asset}","browser_download_url":"https://example.com/a.zip"}}]}}"#
        );

        let (target, discovered) = target_from_api(Ok((body.to_string(), proxy.clone()))).unwrap();
        assert_eq!(target.version, "4.2");
        assert_eq!(
            discovered,
            Some(proxy),
            "API 选出的通道必须回传给下载阶段，否则又要从官方重走一遍（E1）"
        );
        assert!(target.note.is_none());

        let (target, discovered) = target_from_api(Err(InstallAbort::AllFailed(
            "全部下载通道均失败（…）".to_string(),
        )))
        .unwrap();
        assert_eq!(target.version, scrcpy::BEST_FIT_VERSION);
        assert!(discovered.is_none(), "回退时不得携带来源不明的通道");
        assert!(target
            .note
            .as_deref()
            .unwrap()
            .contains("改为下载最佳适配版本"));

        // 取消**不上抛成回退**：它必须原样冒泡，否则用户点了取消却看见一次正常开始的下载。
        assert!(matches!(
            target_from_api(Err(InstallAbort::Canceled)),
            Err(InstallAbort::Canceled)
        ));
    }

    #[test]
    fn prioritize_moves_the_known_good_channel_first() {
        let channels = resolve_channels(&[
            "https://gh-proxy.org".to_string(),
            "https://ghproxy.net/".to_string(),
            "https://mirror.test".to_string(),
        ]);
        assert_eq!(prioritize(&channels, None), channels, "无已知可用通道时原样返回");

        let moved = prioritize(&channels, Some(&channels[2]));
        assert_eq!(moved[0], channels[2], "命中项被挪到首位");
        assert_eq!(moved[1..3], channels[..2], "其余保持原顺序");
        assert_eq!(moved[3], channels[3]);

        let stranger = Channel::Proxy {
            url: "https://not-in-list.test".to_string(),
        };
        assert_eq!(
            prioritize(&channels, Some(&stranger)),
            channels,
            "列表外的通道不得被插入 —— 长度变化会让界面上的「N/M」与实际尝试数脱节"
        );
    }

    // ── 真实网络冒烟（默认忽略，手动运行） ──
    //
    // 运行：`cargo test -- --ignored network_smoke --nocapture`
    // 覆盖「真实发布资产 → 通道降级 → 官方 SHA256SUMS 定位条目 → 校验本地哈希 → 解压
    // → 校验必需文件 → 原子替换到目标目录」这条完整链路（tasks 4.2 / 4.4 / 4.6）。

    /// 冒烟用例共用的通道列表：出厂默认两条，与运行时按配置现算的结果一致。
    fn smoke_proxy_list() -> Vec<String> {
        config::DEFAULT_PROXIES.iter().map(|s| (*s).to_string()).collect()
    }

    #[test]
    #[ignore]
    fn network_smoke_fetches_official_sums() {
        let channels = resolve_channels(&smoke_proxy_list());
        let client = http_client().unwrap();
        let target = best_fit_target().unwrap();
        let ctl = InstallCtl::new();

        let memory = Mutex::new(ChannelMemory::default());
        let (body, channel) = block_on(attempt_channels(&memory, &channels, no_attempt, |c| {
            // 闭包参数与运行期状态都不能直接 `async move`：把通道克隆下来、引用在块外取好。
            let c = (*c).clone();
            let client = &client;
            let target = &target;
            let _ = &ctl;
            async move {
                fetch_sums_with(|name| {
                    let url = sibling_url(&target.url, name).unwrap();
                    let dest = c.url_for(&url);
                    async move {
                        fetch_text(client, &dest)
                            .await
                            .map_err(|failure| failure.reason)
                    }
                })
                .await
                .map_err(AttemptFailure::body_reason)
                .map_err(AttemptError::Failure)
            }
        }))
        .unwrap();
        println!("生效通道：{}", channel.label());

        let hash = parse_sha256sums(&body, &target.asset).expect("SHA256SUMS 应含本次资产条目");
        assert_eq!(hash.len(), 64);
        println!("{} → {}", target.asset, hash);
    }

    #[test]
    #[ignore]
    fn network_smoke_parses_latest_release() {
        // 「检查更新」与「下载最新版本」都依赖真实发布 JSON 的字段结构。
        let channels = resolve_channels(&smoke_proxy_list());
        let client = http_client().unwrap();
        let memory = Mutex::new(ChannelMemory::default());
        let (body, channel) = block_on(attempt_channels(&memory, &channels, no_attempt, |c| {
            let url = c.url_for(LATEST_RELEASE_API);
            let client = &client;
            async move { fetch_text(client, &url).await.map_err(AttemptError::Failure) }
        }))
        .unwrap();
        println!("生效通道：{}", channel.label());

        let target = parse_latest_release(&body).unwrap();
        assert!(target.version.starts_with('4'));
        assert_eq!(target.asset, asset_at(here(), &target.version));
        assert!(target.url.starts_with("https://github.com/Genymobile/scrcpy/releases/download/"));
        assert_eq!(
            classify_update(Some(&target.version), &target.version).0,
            "upToDate"
        );
        assert_eq!(
            classify_update(Some(scrcpy::BEST_FIT_VERSION), &target.version).0,
            if scrcpy::BEST_FIT_VERSION == target.version {
                "upToDate"
            } else {
                "updateAvailable"
            }
        );
        println!("官方最新：{} → {}", target.version, target.url);
    }

    #[test]
    #[ignore]
    fn network_smoke_full_pipeline_into_temp_dir() {
        let channels = resolve_channels(&smoke_proxy_list());
        let client = http_client().unwrap();
        let target = best_fit_target().unwrap();
        let ctl = InstallCtl::new();
        let root = unique_temp_dir("e2e");
        let staging = new_staging_dir(&root).unwrap();
        let zip_path = staging.join(&target.asset);

        let memory = Mutex::new(ChannelMemory::default());
        let (_, channel) = block_on(attempt_channels(&memory, &channels, no_attempt, |c| {
            let c = (*c).clone();
            let client = &client;
            let target = &target;
            let zip_path = &zip_path;
            let ctl = &ctl;
            async move {
                let url = c.url_for(&target.url);
                download_to_file(client, &url, zip_path, ctl, |_, _| {}).await
            }
        }))
        .unwrap();
        println!("生效通道：{}", channel.label());

        let sums = block_on(fetch_sums_with(|name| {
            let url = sibling_url(&target.url, name).unwrap();
            let dest = channel.url_for(&url);
            let client = &client;
            async move {
                fetch_text(client, &dest)
                    .await
                    .map_err(|failure| failure.reason)
            }
        }));
        let local = sha256_file(&zip_path).unwrap();
        match classify_verification(sums, &target.asset, &local) {
            Verification::Verified { expected } => assert_eq!(expected, local),
            other => panic!("真实资产应通过官方校验，实际 {:?}", other),
        }

        let raw = staging.join("raw");
        extract_archive(&zip_path, here(), &raw, |_, _| {}).unwrap();
        let payload = find_payload_root(&raw).unwrap();
        println!("资产根目录：{}", payload.display());
        assert!(assets::missing_bins(&payload).is_empty());

        let installed = root.join("scrcpy-win64-v5.0.1");
        replace_dir(&payload, &installed).unwrap();
        assert!(installed.join(scrcpy::SCRCPY_BIN).is_file());
        assert!(installed.join(assets::ADB_BIN).is_file());
        assert_eq!(
            scrcpy::detect_installed_version(&installed).as_deref(),
            Some(scrcpy::BEST_FIT_VERSION)
        );

        let _ = fs::remove_dir_all(&root);
    }

    // ── 运行时上下文回归（design D1） ──

    /// 客户端在**异步工作线程**上创建与析构应当正常。
    ///
    /// 这是本次迁移的动机之一：**阻塞版**客户端内部持有 tokio 运行时，在异步上下文里析构
    /// 会 panic（历史 bug），因此过去整段下载都得绕到阻塞线程池；async 客户端不自带运行时，
    /// 那条限制消失。同时锁住 `run_blocking` 的两条既有语义。
    #[test]
    fn async_client_and_run_blocking_survive_the_runtime() {
        let on_worker = tauri::async_runtime::block_on(async {
            tauri::async_runtime::spawn(async {
                let client = http_client().unwrap();
                drop(client);
                "ok"
            })
            .await
        });
        assert_eq!(
            on_worker.unwrap(),
            "ok",
            "async 客户端在异步工作线程上创建与析构应当正常"
        );

        // 阻塞工作（解压 / 替换）仍走阻塞线程池，返回值原样带回。
        let on_blocking = tauri::async_runtime::block_on(async {
            run_blocking(|| {
                let client = http_client()?;
                drop(client);
                Ok(7u8)
            })
            .await
        });
        assert_eq!(on_blocking.unwrap(), 7);

        // 阻塞任务的 panic 必须以 Err 形式回传，而不是把进程带走。
        let panicked = tauri::async_runtime::block_on(async {
            run_blocking(|| -> Result<u8, String> { panic!("boom") }).await
        });
        assert!(panicked.is_err());
    }

    // ── 代理地址的持久化与可管理性（design D9） ──

    #[test]
    fn configured_proxies_round_trip_through_config_file() {
        let dir = unique_temp_dir("cfg");
        fs::create_dir_all(&dir).unwrap();
        let mut cfg = AppConfig::fresh();
        // 出厂默认已在列表里；再追加一条，验证两类走同一条路径。
        push_proxy(&mut cfg, "https://mirror.test").unwrap();
        let path = dir.join("config.json");
        fs::write(&path, serde_json::to_vec_pretty(&cfg).unwrap()).unwrap();

        let raw = fs::read_to_string(&path).unwrap();
        let loaded: AppConfig = serde_json::from_str(&raw).unwrap();

        assert_eq!(loaded.channels.proxies, cfg.channels.proxies);
        assert_eq!(loaded.channels.proxies.len(), 3);
        // 重启后全部参与降级：官方直连 + 3 条代理。
        assert_eq!(resolve_channels(&loaded.channels.proxies).len(), 4);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn factory_default_addresses_are_deletable() {
        let mut cfg = AppConfig::fresh();
        assert_eq!(cfg.channels.proxies.len(), 2);

        // 出厂默认的那两条与用户追加项一样删得掉 —— 不存在「内置项不可删」。
        for url in config::DEFAULT_PROXIES {
            remove_proxy(&mut cfg, url).unwrap();
        }
        assert!(cfg.channels.proxies.is_empty());
        assert_eq!(
            resolve_channels(&cfg.channels.proxies).len(),
            1,
            "删光之后只剩 GitHub 直连这一条通道"
        );

        // 删掉就是删掉：没有「恢复默认」，再删一次只会说它不在列表里。
        assert!(remove_proxy(&mut cfg, config::DEFAULT_PROXIES[0]).is_err());
        assert!(cfg.channels.proxies.is_empty());
    }

    #[test]
    fn adding_a_duplicate_or_invalid_address_is_rejected() {
        let mut cfg = AppConfig::fresh();

        // 出厂默认地址已在列表里 ⇒ 重复追加被拒（且不再有「已内置」这条特例）。
        assert!(push_proxy(&mut cfg, "https://gh-proxy.org/").is_err());
        assert!(push_proxy(&mut cfg, "https://gh-proxy.org").is_err());
        assert!(push_proxy(&mut cfg, "gh-proxy.org").is_err());
        assert_eq!(cfg.channels.proxies.len(), 2, "被拒的追加不得改动列表");

        push_proxy(&mut cfg, "https://mirror.test").unwrap();
        assert_eq!(cfg.channels.proxies.len(), 3);
        assert!(push_proxy(&mut cfg, "https://mirror.test").is_err());
    }
}
