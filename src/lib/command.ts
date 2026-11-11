// 参数合并与命令拼装（design D5 的合并规则 + tasks 8.1 的拼装）。
//
// 合并规则只有一条：`enabled ? merge(全局参数, 设备参数取值) : 全局参数`。
// 设备参数处于停用状态时调用方传 null —— 取值仍留在盘上，但不参与拼装（design D5）。
//
// 拼装只使用 scrcpy 自身参数，且**跳过校验失败的字段**（tasks 7.3：
// 非法值 MUST NOT 用于发起会话）。「未设置」与「等于内置默认值」的项不产生 flag
// （`--window-title` 例外：它的判据只有「非空」，见下方窗口组），
// 这样预览里的命令就是真正会执行的那条，且与界面上的改动一一对应。

import { BUILTIN_DEFAULTS } from './defaults';
import type { Defaults } from './types';
import { invalidKeys, isBlank, type ParamErrors } from './validate';

/** 按「组 → 键 → 值」组织的参数快照。 */
export type ParamValues = Record<string, Record<string, unknown>>;

/**
 * 参数合并：`over` 中的每一项逐层盖在 `base` 之上（组内按键覆盖，不整组替换）。
 *
 * 同时用于两处 —— 设备参数叠加到全局参数（D5），以及把内置默认值补到盘上读来的
 * 参数之下（保证字段模型新增的键也有值）。
 */
export function mergeParams(
  base: Record<string, unknown>,
  over: Record<string, unknown> | null | undefined,
): Defaults {
  // 取值都是 JSON 安全的标量 / 数组，深拷贝用序列化即可（不引入额外依赖）。
  const out = JSON.parse(JSON.stringify(base ?? {})) as ParamValues;
  if (!over) return out as unknown as Defaults;
  for (const [group, fields] of Object.entries(over)) {
    if (!fields || typeof fields !== 'object' || Array.isArray(fields)) continue;
    out[group] = { ...(out[group] ?? {}), ...(fields as Record<string, unknown>) };
  }
  return out as unknown as Defaults;
}

function str(v: unknown): string {
  if (v === null || v === undefined) return '';
  return String(v).trim();
}

function num(v: unknown): number {
  const n = typeof v === 'number' ? v : Number(str(v));
  return Number.isFinite(n) ? n : 0;
}

function bool(v: unknown): boolean {
  return v === true;
}

/** 一条待下发的参数，连同产生它的字段地址（`group.key`）。 */
interface ArgEntry {
  ref: string;
  text: string;
}

/** 应用会以短名输出的两项：与长名算同一个 scrcpy 选项。 */
const SHORT_FLAGS: Record<string, string> = {
  '-b': '--video-bit-rate',
  '-m': '--max-size',
};

/** 参数文本的选项名：`--name=value` → `--name`；短名归一为长名；不是选项时返回 `null`。 */
function flagOf(text: string): string | null {
  const head = text.trim().split(/\s+/, 1)[0] ?? '';
  if (!/^--?[A-Za-z]/.test(head)) return null;
  const name = head.split('=')[0];
  return SHORT_FLAGS[name] ?? name;
}

/**
 * 自定义参数切分：空白（含换行）分隔；引号内的空白保留在同一个参数内，
 * 引号本身不进 argv；反斜杠按普通字符处理（Windows 路径直接可用）。
 * 引号不成对时整段已被校验拦下，不会走到这里。
 */
export function splitCustomArgs(raw: string): string[] {
  const out: string[] = [];
  let cur = '';
  let quoted = false;
  let quote: '"' | "'" | null = null;
  for (const ch of raw) {
    if (quote) {
      if (ch === quote) quote = null;
      else cur += ch;
      continue;
    }
    if (ch === '"' || ch === "'") {
      quote = ch;
      quoted = true;
      continue;
    }
    if (/\s/.test(ch)) {
      if (cur || quoted) {
        out.push(cur);
        cur = '';
        quoted = false;
      }
      continue;
    }
    cur += ch;
  }
  if (cur || quoted) out.push(cur);
  return out;
}

/**
 * 跨来源去重：设备参数启用时，设备侧（字段或额外参数）已下发的同名 flag，
 * 全局侧的同一项不再保留。同一侧内部的重复保持原样（按后写生效）。
 * `deviceKeys` 是设备覆盖里真正有取值的 `group.key`；空集合表示不去重。
 */
function dedupeArgs(entries: ArgEntry[], deviceKeys?: ReadonlySet<string>): string[] {
  if (!deviceKeys || deviceKeys.size === 0) return entries.map((e) => e.text);
  const deviceFlags = new Set<string>();
  for (const e of entries) {
    if (!deviceKeys.has(e.ref)) continue;
    const flag = flagOf(e.text);
    if (flag) deviceFlags.add(flag);
  }
  const kept: ArgEntry[] = [];
  for (let i = 0; i < entries.length; i += 1) {
    const e = entries[i];
    if (deviceKeys.has(e.ref)) {
      kept.push(e);
      continue;
    }
    const flag = flagOf(e.text);
    if (!flag || !deviceFlags.has(flag)) {
      kept.push(e);
      continue;
    }
    // 空格形式（`--name value`）的重复项：值 token 跟随剔除。
    if (e.ref === 'advanced.customArgs') {
      const next = entries[i + 1];
      if (next && next.ref === e.ref && flagOf(next.text) === null) i += 1;
    }
  }
  return kept.map((e) => e.text);
}

/** 命令的三段：可执行文件、`-s` 的连接标识、其余参数（供预览分段着色）。 */
export interface CommandParts {
  bin: string;
  serial: string;
  args: string[];
}

/**
 * 拼装参数列表（tasks 8.1）。
 *
 * `errors` 里出现过的字段（`group.key`，见 `validate.ts`）会被整体跳过 ——
 * 非法值不进命令，界面另给出提示（tasks 7.3）。
 *
 * `deviceKeys` 是该设备覆盖里真正有取值的 `group.key`；传入时做跨来源去重
 * （设备侧的 flag 一经下发，全局侧的同一项就不再出现）。
 */
export function buildArgs(
  p: Defaults,
  errors?: ParamErrors | Set<string>,
  deviceKeys?: ReadonlySet<string>,
): string[] {
  const invalid = errors instanceof Set ? errors : errors ? invalidKeys(errors) : null;
  const bad = (group: string, key: string) => !!invalid && invalid.has(`${group}.${key}`);
  const groups = p as unknown as ParamValues;
  const g = (name: string) => groups[name] ?? {};
  const builtin = BUILTIN_DEFAULTS as unknown as ParamValues;

  const v = g('video');
  const s = g('source');
  const au = g('audio');
  const c = g('control');
  const r = g('record');
  const w = g('window');
  const ap = g('apps');
  const adv = g('advanced');
  const entries: ArgEntry[] = [];
  const add = (ref: string, text: string) => entries.push({ ref, text });

  // Flex Display 是否真的生效：它只在「视频源为显示器」且「显示模式为 flex」时下发（见下方显示源
  // 一节）。判据必须含「视频源不是摄像头」这一半 —— 摄像头源下 flex 分支根本不会走到，若只看
  // displayMode，那四项与 flex 互斥的参数会被误剔（用户的取值仍在，只是不该被这次剔除带走）。
  const isFlex = str(s.videoSource) !== 'camera' && str(s.displayMode) === 'flex';
  // 摄像头源下「关闭屏幕」不生效（**应用侧口径**，与 isFlex 同读 source 组取值）：scrcpy 自己
  // 并不拒绝这个组合 —— doc/scrcpy 的 app/src/cli.c 对 --turn-screen-off 只约束「设备控制被关闭」
  // （:3420-3424）与 OTG 模式（:3470-3472），摄像头分支（:3209-3257）只管 --display-id /
  // --display-ime-policy / --camera-*。故不靠它报错，由应用在拼装层剔除、在参数说明里点明；
  // 取值原样保留在配置里（同 isFlex 那几项），视频源切回主屏幕后照常下发。
  const isCamera = str(s.videoSource) === 'camera';

  // ── 视频（--video-codec / --video-encoder / --hwdec / --video-bit-rate / --max-size /
  //    --max-fps / --crop / --angle / --render-fit / --video-buffer，见 doc/scrcpy/app/src/cli.c）──
  if (str(v.videoCodec) && !bad('video', 'videoCodec')) {
    add('video.videoCodec', `--video-codec=${str(v.videoCodec)}`);
  }
  if (str(v.videoEncoder) && !bad('video', 'videoEncoder')) {
    add('video.videoEncoder', `--video-encoder=${str(v.videoEncoder)}`);
  }
  // `--hwdec` 与「后台录制」互斥：`--no-window` 隐含 `--no-video-playback`
  // （doc/scrcpy 的 app/src/cli.c 帮助文本原文），而硬件解码要求 video playback ——
  // 该组合会被 scrcpy 以 "Hardware decoding requires video playback" 直接拒绝。
  // 故后台录制下只放行 `disabled`（合法），显式硬件模式剔除但取值保留。
  if (str(v.hwdec) && !bad('video', 'hwdec')) {
    if (str(v.hwdec) === 'disabled' || !bool(r.noWindow)) {
      add('video.hwdec', `--hwdec=${str(v.hwdec)}`);
    }
  }
  if (num(v.bitrate) > 0 && !bad('video', 'bitrate')) {
    add('video.bitrate', `-b ${num(v.bitrate)}M`);
  }
  if (num(v.maxSize) > 0 && !bad('video', 'maxSize')) {
    add('video.maxSize', `-m ${num(v.maxSize)}`);
  }
  if (num(v.maxFps) > 0 && !bad('video', 'maxFps')) {
    add('video.maxFps', `--max-fps=${num(v.maxFps)}`);
  }
  // `--crop` 与 Flex Display 不兼容（doc/scrcpy 的 app/src/cli.c:3277）：flex 档不下发它，
  // 但用户在配置里的取值原样保留（换回别的显示模式后照常生效）。
  if (str(v.crop) && !bad('video', 'crop') && !isFlex) {
    add('video.crop', `--crop=${str(v.crop)}`);
  }
  if (str(v.angle) !== '0' && !bad('video', 'angle')) {
    add('video.angle', `--angle=${str(v.angle)}`);
  }
  if (str(v.renderFit) && str(v.renderFit) !== 'letterbox' && !bad('video', 'renderFit')) {
    add('video.renderFit', `--render-fit=${str(v.renderFit)}`);
  }
  if (num(v.videoBuffer) > 0 && !bad('video', 'videoBuffer')) {
    add('video.videoBuffer', `--video-buffer=${num(v.videoBuffer)}`);
  }

  // ── 显示源（--video-source / --camera-* / --new-display / --no-vd-destroy-content /
  //    --flex-display / --display-id）──
  //
  // 「虚拟显示器」与「Flex Display」共用一段拼装：Flex **不是**第三种显示模式，而是虚拟显示器的
  // 一个修饰开关（scrcpy 的 `-x`）—— 它必须建立在 `--new-display` 之上（doc/scrcpy 的
  // app/src/cli.c:3264-3270），且 flex 下尺寸只能由 `--new-display=WxH` 指定（同文件 :3282 的报错
  // 信息），故两档只差一个 `--flex-display`。旧写法在这里推 `--display-id=0 --flex-display`，
  // 缺了必需的 `--new-display`，scrcpy 会直接报错退出（`--display-id=0` 本身也无作用：0 就是默认值）。
  if (str(s.videoSource) === 'camera') {
    add('source.videoSource', '--video-source=camera');
    if (str(s.cameraFacing)) {
      add('source.cameraFacing', `--camera-facing=${str(s.cameraFacing)}`);
    }
    if (str(s.cameraSize) && !bad('source', 'cameraSize')) {
      add('source.cameraSize', `--camera-size=${str(s.cameraSize)}`);
    }
    if (num(s.cameraFps) > 0 && !bad('source', 'cameraFps')) {
      add('source.cameraFps', `--camera-fps=${num(s.cameraFps)}`);
    }
    if (num(s.cameraZoom) !== 1 && num(s.cameraZoom) > 0 && !bad('source', 'cameraZoom')) {
      add('source.cameraZoom', `--camera-zoom=${num(s.cameraZoom)}`);
    }
    if (bool(s.cameraTorch)) add('source.cameraTorch', '--camera-torch');
  } else if (str(s.displayMode) === 'new' || isFlex) {
    // `--new-display[=WxH][/DPI]`：尺寸与密度两个子项都可缺省。**「本为空」与「格式非法」都按
    // 空处理**（该子项不参与），但 flag 本身照发 —— 「是不是虚拟显示器 / Flex」由 displayMode 决定，
    // 不该因为某一项取值非法就整条不下发，让人以为自己在镜像主屏。摄像头分支同样是逐条判空，
    // 取向一致（见上面四行）。
    const res = bad('source', 'newDisplayRes') ? '' : str(s.newDisplayRes);
    const dpi = bad('source', 'newDisplayDpi') ? '' : str(s.newDisplayDpi);
    const spec = res || dpi ? `${res}${dpi ? `/${dpi}` : ''}` : '';
    add('source.displayMode', spec ? `--new-display=${spec}` : '--new-display');
    if (bool(s.noVdDestroy)) add('source.noVdDestroy', '--no-vd-destroy-content');
    if (isFlex) add('source.displayMode', '--flex-display');
  } else {
    // 主屏镜像：`--display-id` 只在此时有效 —— 非 0 与 `--new-display` 互斥
    // （scrcpy 报 "Cannot specify both --display-id and --new-display"），摄像头源下
    // 该 flag 也不可用（"only available with --video-source=display"）。另外两个分支
    // 都不下发它，取值原样保留在配置里（同 flex / 摄像头那几项的剔除模式）。
    if (num(s.displayId) > 0 && !bad('source', 'displayId')) {
      add('source.displayId', `--display-id=${num(s.displayId)}`);
    }
  }

  // ── 音频（--no-audio / --audio-source / --audio-codec / --audio-encoder / --audio-dup /
  //    --audio-bit-rate / --audio-buffer / --audio-output-buffer）──
  // 开关语义是「禁用音频转发」（出厂即开），因此开 ⇒ 下发 `--no-audio`。
  if (bool(au.audioOff)) {
    add('audio.audioOff', '--no-audio');
  } else {
    if (str(au.audioSource) && str(au.audioSource) !== 'output') {
      add('audio.audioSource', `--audio-source=${str(au.audioSource)}`);
    }
    if (str(au.audioCodec) && str(au.audioCodec) !== 'opus') {
      add('audio.audioCodec', `--audio-codec=${str(au.audioCodec)}`);
    }
    if (str(au.audioEncoder) && !bad('audio', 'audioEncoder')) {
      add('audio.audioEncoder', `--audio-encoder=${str(au.audioEncoder)}`);
    }
    if (bool(au.audioDup)) add('audio.audioDup', '--audio-dup');
    if (num(au.audioBitrate) > 0 && !bad('audio', 'audioBitrate')) {
      add('audio.audioBitrate', `--audio-bit-rate=${num(au.audioBitrate)}K`);
    }
    // 与内置默认值相同则不出现在命令里（scrcpy 自身默认值与这里一致）。
    if (
      num(au.audioBuffer) > 0 &&
      num(au.audioBuffer) !== num(builtin.audio?.audioBuffer) &&
      !bad('audio', 'audioBuffer')
    ) {
      add('audio.audioBuffer', `--audio-buffer=${num(au.audioBuffer)}`);
    }
    if (
      num(au.audioOutBuffer) > 0 &&
      num(au.audioOutBuffer) !== num(builtin.audio?.audioOutBuffer) &&
      !bad('audio', 'audioOutBuffer')
    ) {
      add('audio.audioOutBuffer', `--audio-output-buffer=${num(au.audioOutBuffer)}`);
    }
  }

  // ── 控制（--no-control / --keyboard / --no-key-repeat / --mouse / --gamepad /
  //    --shortcut-mod / --show-touches / --turn-screen-off / --screen-off-timeout /
  //    --stay-awake / --keep-active / --power-off-on-close / --no-clipboard-autosync）──
  // `-n/--no-control` 与 Flex Display 不兼容（doc/scrcpy 的 app/src/cli.c:3272）：flex 档不下发。
  if (bool(c.noControl) && !isFlex) add('control.noControl', '--no-control');
  // 输入模式只在取值与 scrcpy 默认不同时下发（默认：键盘 / 鼠标 `sdk`、手柄 `disabled`）。
  // 判据比的是内置默认 —— 它是 scrcpy 默认的镜像；用户改过盘上的默认参数也不改变这一点。
  if (str(c.keyboard) && str(c.keyboard) !== str(builtin.control?.keyboard)) {
    add('control.keyboard', `--keyboard=${str(c.keyboard)}`);
  }
  if (bool(c.noKeyRepeat)) add('control.noKeyRepeat', '--no-key-repeat');
  if (str(c.mouse) && str(c.mouse) !== str(builtin.control?.mouse)) {
    add('control.mouse', `--mouse=${str(c.mouse)}`);
  }
  if (str(c.gamepad) && str(c.gamepad) !== str(builtin.control?.gamepad)) {
    add('control.gamepad', `--gamepad=${str(c.gamepad)}`);
  }
  const mods = Array.isArray(c.shortMod) ? (c.shortMod as unknown[]).map(str).filter(Boolean) : [];
  if (mods.length) add('control.shortMod', `--shortcut-mod=${mods.join(',')}`);
  if (bool(c.showTouches)) add('control.showTouches', '--show-touches');
  // 摄像头源下剔除（取值仍留在配置里，切回主屏幕后照常生效，同上面那四项 flex 剔除）。
  if (bool(c.turnScreenOff) && !isCamera) {
    add('control.turnScreenOff', '--turn-screen-off');
  }
  if (num(c.screenOffTimeout) > 0 && !bad('control', 'screenOffTimeout')) {
    add('control.screenOffTimeout', `--screen-off-timeout=${num(c.screenOffTimeout)}`);
  }
  if (bool(c.stayAwake)) add('control.stayAwake', '--stay-awake');
  if (bool(c.keepActive)) add('control.keepActive', '--keep-active');
  if (bool(c.powerOffOnClose)) add('control.powerOffOnClose', '--power-off-on-close');
  // 开关语义是「禁用双向同步」：开 ⇒ 下发该 flag（出厂不选中）。
  if (bool(c.clipboardOff)) add('control.clipboardOff', '--no-clipboard-autosync');

  // ── 录制（--record / --record-format / --record-orientation / --no-window /
  //    --time-limit / --pause-on-exit）──
  if (bool(r.recordOn)) {
    if (str(r.recordPath)) add('record.recordPath', `--record=${str(r.recordPath)}`);
    if (str(r.recordFormat)) add('record.recordFormat', `--record-format=${str(r.recordFormat)}`);
  }
  if (str(r.recordOrientation) !== '0' && str(r.recordOrientation)) {
    add('record.recordOrientation', `--record-orientation=${str(r.recordOrientation)}`);
  }
  if (bool(r.noWindow)) add('record.noWindow', '--no-window');
  if (num(r.timeLimit) > 0 && !bad('record', 'timeLimit')) {
    add('record.timeLimit', `--time-limit=${num(r.timeLimit)}`);
  }
  if (str(r.pauseOnExit) && str(r.pauseOnExit) !== 'false') {
    add('record.pauseOnExit', `--pause-on-exit=${str(r.pauseOnExit)}`);
  }

  // ── 窗口（--window-title / --window-borderless / --fullscreen / --always-on-top /
  //    --no-window-aspect-ratio-lock / --background-color / --window-width|height|x|y）──
  // `--window-title` 的判据是「非空即传」（出厂值为空 ⇒ 标题交给 scrcpy 用设备名）。
  // 不与出厂值比对：出厂值本就是空串时两者等价，而保留比对会把用户手动填回的旧出厂文本静默漏发。
  if (str(w.windowTitle)) {
    add('window.windowTitle', `--window-title=${str(w.windowTitle)}`);
  }
  if (bool(w.borderless)) add('window.borderless', '--window-borderless');
  if (bool(w.fullscreen)) add('window.fullscreen', '--fullscreen');
  if (bool(w.alwaysOnTop)) add('window.alwaysOnTop', '--always-on-top');
  // 开关语义是「禁用宽高比锁定」：开 ⇒ 下发该 flag（出厂不选中）。
  if (bool(w.aspectLockOff)) add('window.aspectLockOff', '--no-window-aspect-ratio-lock');
  if (str(w.bgColor) && str(w.bgColor) !== str(builtin.window?.bgColor)) {
    add('window.bgColor', `--background-color=${str(w.bgColor)}`);
  }
  // `--window-width` / `--window-height` 与 Flex Display 不兼容（doc/scrcpy 的
  // app/src/cli.c:3282，报错信息指向「要指定尺寸请用 `--new-display=WxH`」）：flex 档不下发，
  // 取值原样保留在配置里。`--window-x` / `--window-y` 不在互斥之列，照常下发。
  if (num(w.windowWidth) > 0 && !bad('window', 'windowWidth') && !isFlex) {
    add('window.windowWidth', `--window-width=${num(w.windowWidth)}`);
  }
  if (num(w.windowHeight) > 0 && !bad('window', 'windowHeight') && !isFlex) {
    add('window.windowHeight', `--window-height=${num(w.windowHeight)}`);
  }
  if (num(w.windowX) > 0 && !bad('window', 'windowX')) {
    add('window.windowX', `--window-x=${num(w.windowX)}`);
  }
  if (num(w.windowY) > 0 && !bad('window', 'windowY')) {
    add('window.windowY', `--window-y=${num(w.windowY)}`);
  }

  // ── 应用（--start-app）──
  if (str(ap.startApp)) add('apps.startApp', `--start-app=${str(ap.startApp)}`);

  // ── 更多（--verbosity / --print-fps / --no-cleanup / --port / --tunnel-host）──
  if (str(adv.verbosity) && str(adv.verbosity) !== str(builtin.advanced?.verbosity)) {
    add('advanced.verbosity', `--verbosity=${str(adv.verbosity)}`);
  }
  if (bool(adv.printFps)) add('advanced.printFps', '--print-fps');
  // 开关语义是「禁用退出清理」：开 ⇒ 下发该 flag（出厂不选中）。
  if (bool(adv.cleanupOff)) add('advanced.cleanupOff', '--no-cleanup');
  if (
    !isBlank(adv.port) &&
    str(adv.port) !== str(builtin.advanced?.port) &&
    !bad('advanced', 'port')
  ) {
    add('advanced.port', `--port=${str(adv.port)}`);
  }
  if (
    !isBlank(adv.tunnelHost) &&
    str(adv.tunnelHost) !== str(builtin.advanced?.tunnelHost) &&
    !bad('advanced', 'tunnelHost')
  ) {
    add('advanced.tunnelHost', `--tunnel-host=${str(adv.tunnelHost)}`);
  }

  // 自定义参数：按空白切成多个参数（引号内空白保留），逐个追加 ——
  // 界面上没有提供的新 flag 因此也能用，UI 不绑定 scrcpy 版本。
  const custom = str(adv.customArgs);
  if (custom && !bad('advanced', 'customArgs')) {
    for (const token of splitCustomArgs(custom)) add('advanced.customArgs', token);
  }

  return dedupeArgs(entries, deviceKeys);
}

/** 拼装完整命令的三段（预览据此着色，复制时拼接为纯文本）。 */
export function buildCommandParts(
  connectionId: string,
  p: Defaults,
  errors?: ParamErrors | Set<string>,
  deviceKeys?: ReadonlySet<string>,
): CommandParts {
  return { bin: 'scrcpy', serial: connectionId, args: buildArgs(p, errors, deviceKeys) };
}

/** 纯文本形式的命令（复制用；不含任何界面标记）。 */
export function commandText(parts: CommandParts): string {
  return [parts.bin, '-s', parts.serial, ...parts.args].join(' ');
}
