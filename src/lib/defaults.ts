import type { Defaults } from './types';

/**
 * 内置默认参数（与 ui-design/launcher-prototype.html 中 `defaults` 一致）。
 * 后端 config.json 的 `defaults` 为空（Null）时，前端以此作为出厂默认，
 * 并在首次保存时写回磁盘（design D7）。flag 名称均对照 doc/scrcpy 源码。
 *
 * 出厂默认的原则是「**不替用户预设**」（change `defaults-tuning`）：能不传的参数就不传，
 * 让 scrcpy 自身的默认行为生效。因此音频转发默认关闭（出厂即下发 `--no-audio`）、
 * `--shortcut-mod` 默认不设置、自定义参数默认为空。**数值型同样不预设**：`--video-codec`
 * 取空串、`--video-bit-rate` 与 `--audio-bit-rate` 取 `0`（即界面上的「自动」），
 * 三者出厂状态下都不产生对应 flag，编码与码率由 scrcpy 自身默认决定。
 *
 * **显示源组同样不预设**（change `source-params-no-preset-and-flex-fix`）：尺寸、密度、摄像头
 * 朝向、摄像头尺寸与帧率都取空串。它的空值语义**分两类**，别混同 —— `--new-display` 取空时仍会
 * 下发该 flag、只是不带取值（该 flag 是否存在由显示模式决定），其余四个取空则不产生各自的 flag。
 *
 * 注意：盘上已存在 `defaults` 时它优先（`store.ts::defaults` 的合并顺序），
 * 改动本文件的出厂值不会覆盖用户已保存过的取值。
 */
export const BUILTIN_DEFAULTS: Defaults = {
  video: {
    // 「自动」：空串 / 0 都不产生对应 flag，编码与码率交给 scrcpy 自身默认。
    videoCodec: '',
    bitrate: 0,
    maxSize: 0,
    maxFps: 0,
    crop: '',
    angle: '0',
    renderFit: 'letterbox',
    videoBuffer: 0,
  },
  source: {
    videoSource: 'display',
    displayMode: 'main',
    // 尺寸与密度留空 ⇒ 只下发**不带取值**的 `--new-display`（该 flag 是否存在由显示模式决定），
    // 尺寸与密度交给 scrcpy：非 Flex 场合用主屏尺寸与密度，Flex Display 场合用 1280x960/160
    // （doc/scrcpy 的 server/.../video/NewDisplayCapture.java:99-127）。
    newDisplayRes: '',
    newDisplayDpi: '',
    noVdDestroy: true,
    // 摄像头组同样不预设：朝向留空 = 不传 `--camera-facing`（界面把它标注为「自动」，落盘值是
    // 空串），由 scrcpy 用检测到的第一个摄像头；尺寸留空 = 由 scrcpy 自动选择 —— 这还顺带解掉
    // 一处必然冲突（`--camera-size` 与 `-m`/`--max-size` 互斥，doc/scrcpy/doc/camera.md:107）；
    // 帧率留空 = 用 Android 默认的 30 fps。
    cameraFacing: '',
    cameraSize: '',
    cameraFps: '',
    cameraZoom: 1,
    cameraTorch: false,
  },
  audio: {
    // 出厂不转发设备音频：默认转发会让部分拒绝 AudioRecord 的机型一开会话就退出，
    // 而这属于用户没做任何操作就会踩到的默认值问题。
    // 该开关语义是「禁用音频转发」（开 ⇒ 下发 `--no-audio`），故出厂值取 `true`。
    audioOff: true,
    audioSource: 'output',
    audioCodec: 'opus',
    audioDup: false,
    // 「自动」= 不传 `--audio-bit-rate`（与视频码率同形）。
    audioBitrate: 0,
    audioBuffer: 50,
    audioOutBuffer: 10,
  },
  control: {
    noControl: false,
    keyboard: 'uhid',
    mouse: 'uhid',
    gamepad: 'uhid',
    // 空数组 = 不产生 `--shortcut-mod`，快捷键沿用 scrcpy 自身的默认修饰键。
    shortMod: [],
    showTouches: false,
    turnScreenOff: false,
    stayAwake: false,
    keepActive: false,
    powerOffOnClose: false,
    clipboard: true,
  },
  record: {
    recordOn: false,
    recordPath: 'D:/Recordings/session.mp4',
    recordFormat: 'mp4',
    recordOrientation: '0',
    noWindow: false,
    timeLimit: 0,
    pauseOnExit: 'false',
  },
  window: {
    // 出厂为空：不下发 `--window-title`，窗口标题由 scrcpy 用设备名（见 doc/scrcpy `app/src/scrcpy.c`）。
    windowTitle: '',
    borderless: false,
    fullscreen: false,
    alwaysOnTop: false,
    aspectLock: true,
    bgColor: '#222222',
    windowWidth: 0,
    windowHeight: 0,
    windowX: 0,
    windowY: 0,
  },
  apps: {
    startApp: '',
  },
  advanced: {
    // 出厂为空：不向最终命令追加任何内容（透传框由用户按需填写）。
    customArgs: '',
    verbosity: 'info',
    printFps: false,
    cleanup: true,
    port: '27183:27199',
    tunnelHost: 'localhost',
  },
};
