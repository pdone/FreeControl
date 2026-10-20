// 8 组默认参数的数据驱动描述（对照 ui-design/launcher-prototype.html 与 doc/scrcpy）。
// 表单控件据此渲染；flag 名称均来自 scrcpy 源码。前端拥有字段模型（design D5/D7）。

export type FieldType =
  | 'toggle'
  | 'segmented'
  | 'select'
  | 'text'
  | 'number'
  | 'range'
  | 'chips'
  | 'textarea';

export interface FieldOption {
  label: string;
  value: string;
}

export interface ParamField {
  key: string;
  label: string;
  type: FieldType;
  options?: FieldOption[];
  min?: number;
  max?: number;
  step?: number;
  /** 数值单位，如 Mbps / Kbps / px。 */
  unit?: string;
  /** 当数值为 0（或空）时显示的替代文案，如「自动」「不限制」。 */
  autoZero?: string;
  placeholder?: string;
  hint?: string;
  /**
   * 详情段落：较长、或带前置条件 / 平台限制的说明按语义拆段，收进标签右侧 `?` 的详情面板
   * （`HelpTip.svelte`，展开全由 CSS 承担）；字段下方只留一句短 `hint`。
   * 分配口径见 config.yaml ④：常驻短句与详情之间的分配可变，但信息不得丢。
   */
  detail?: string[];
  /** hint 中以警示色呈现的片段（如可读性不足）。 */
  hintWarn?: string;
  /**
   * 字段级显隐：入参是该字段**所在组**的取值（与卡片级 `ParamCard.showWhen` 同源）。
   * 用于「取值在这个状态下不参与拼装，于是干脆不显示它」的字段 —— 显示着一个不生效的
   * 控件，比把它藏起来更容易让人以为设置丢了（如视频源为摄像头时的「显示模式」）。
   */
  showWhen?: (g: Record<string, unknown>) => boolean;
}

export interface ParamCard {
  title: string;
  span2?: boolean;
  showWhen?: (g: Record<string, unknown>) => boolean;
  fields: ParamField[];
}

export interface ParamGroup {
  key: string;
  title: string;
  subtitle: string;
  cards: ParamCard[];
}

const SEG_ANGLE: FieldOption[] = [
  { label: '0', value: '0' },
  { label: '90', value: '90' },
  { label: '180', value: '180' },
  { label: '270', value: '270' },
];

export const PARAM_GROUPS: ParamGroup[] = [
  {
    key: 'source',
    title: '显示源',
    subtitle:
      '镜像主屏、虚拟显示器、Flex Display 或摄像头画面。',
    cards: [
      {
        title: '视频源',
        fields: [
          {
            key: 'videoSource',
            label: '--video-source',
            type: 'segmented',
            options: [
              { label: '主屏幕', value: 'display' },
              { label: '摄像头', value: 'camera' },
            ],
          },
          {
            key: 'displayMode',
            label: '显示模式',
            type: 'segmented',
            options: [
              { label: '主屏', value: 'main' },
              { label: '虚拟显示器', value: 'new' },
              { label: 'Flex Display', value: 'flex' },
            ],
            // 视频源为摄像头时这一档不参与拼装（camera 分支优先），显示出来只会让人以为
            // 设置生效了 —— 与原型 `toggleSourceUI()` 的行为一致。
            showWhen: (g) => g.videoSource !== 'camera',
            hint: '虚拟显示器可指定分辨率与 DPI；Flex Display 让虚拟屏随窗口缩放，尺寸也在此指定（此时 --crop 与窗口宽高不生效）',
          },
          {
            key: 'displayId',
            label: '--display-id',
            type: 'number',
            min: 0,
            max: 9999,
            // 只在镜像主屏时生效：虚拟显示器 / Flex 下 scrcpy 拒绝「两者同时指定」
            // （Cannot specify both --display-id and --new-display），摄像头源下该 flag 不可用。
            // 取值仍保留在配置里，切回主屏后照常生效（与 flex / 摄像头那几项的剔除同一模式）。
            showWhen: (g) => g.videoSource !== 'camera' && g.displayMode === 'main',
            hint: '0 / 留空 = 主屏；虚拟显示器与摄像头源下不生效',
            detail: [
              '镜像指定 id 的设备显示。可用 scrcpy --list-displays 列出可用 id。',
              '非 0 值与 --new-display 互斥（scrcpy 拒绝两者同时指定）；摄像头源下该参数不可用。',
            ],
          },
        ],
      },
      {
        title: '虚拟显示器',
        // Flex Display 是「虚拟显示器 + 一个修饰开关」，两者共用这张卡片：flex 下尺寸只能由
        // `--new-display=WxH` 指定（scrcpy 的报错信息也是这么写的）；摄像头源下两档都不成立。
        showWhen: (g) =>
          g.videoSource !== 'camera' && (g.displayMode === 'new' || g.displayMode === 'flex'),
        fields: [
          {
            key: 'newDisplayRes',
            label: '--new-display',
            type: 'text',
            placeholder: '1920x1080',
            hint: '留空由 scrcpy 决定：非 Flex 用主屏尺寸与密度，Flex 用 1280x960/160',
          },
          {
            key: 'newDisplayDpi',
            label: '密度 DPI',
            type: 'number',
            min: 72,
            max: 1000,
            hint: '留空用上面的默认密度',
          },
          {
            key: 'noVdDestroy',
            label: '--no-vd-destroy-content',
            type: 'toggle',
            hint: '移到主屏而非销毁',
          },
        ],
      },
      {
        title: '摄像头',
        showWhen: (g) => g.videoSource === 'camera',
        fields: [
          {
            key: 'cameraFacing',
            label: '--camera-facing',
            type: 'select',
            // 「自动」= 不传该 flag（空串沿用既有的「未设置」语义，拼装层零改动），由 scrcpy 用
            // 检测到的第一个摄像头。MUST NOT 把界面上这个「自动」当成取值写进配置：scrcpy 只认
            // front / back / external，其余（含界面文案本身）一律判为非法取值。
            options: [
              { label: '自动', value: '' },
              { label: 'front', value: 'front' },
              { label: 'back', value: 'back' },
              { label: 'external', value: 'external' },
            ],
          },
          {
            key: 'cameraSize',
            label: '--camera-size',
            type: 'text',
            placeholder: '1920x1080',
            hint: '留空由 scrcpy 自动选择尺寸',
          },
          {
            key: 'cameraFps',
            label: '--camera-fps',
            type: 'number',
            min: 1,
            max: 240,
            placeholder: '30',
            hint: '留空用 Android 默认的 30 fps',
          },
          { key: 'cameraZoom', label: '--camera-zoom', type: 'number', min: 0.1, max: 10, step: 0.1 },
          {
            key: 'cameraTorch',
            label: '--camera-torch',
            type: 'toggle',
            hint: '启动时点亮',
          },
        ],
      },
    ],
  },
  {
    key: 'video',
    title: '视频',
    subtitle: '编码、分辨率、码率、帧率与画面适配。',
    cards: [
      {
        title: '编码与画质',
        fields: [
          {
            key: 'videoCodec',
            label: '--video-codec',
            type: 'select',
            options: [
              // 「自动」= 不传该 flag，编码由 scrcpy 自身默认决定（见 change `defaults-tuning`），
              // 且出厂默认即「自动」。空串沿用既有的「未设置」语义，拼装层无需为它加分支。
              { label: '自动', value: '' },
              { label: 'h264', value: 'h264' },
              { label: 'h265', value: 'h265' },
              { label: 'av1', value: 'av1' },
              { label: 'vp8', value: 'vp8' },
              { label: 'vp9', value: 'vp9' },
            ],
          },
          {
            key: 'maxSize',
            label: '--max-size',
            type: 'range',
            min: 0,
            max: 2560,
            step: 64,
            unit: 'px',
            // 最左端即「自动」（= 不限制），不再另写一行提示。
            autoZero: '自动',
          },
          {
            key: 'bitrate',
            label: '--video-bit-rate',
            type: 'range',
            min: 0,
            max: 64,
            step: 1,
            unit: 'Mbps',
            // 最左端即「自动」= 不传 `-b`，码率由 scrcpy 自身默认决定；出厂默认即「自动」。
            autoZero: '自动',
          },
          {
            key: 'maxFps',
            label: '--max-fps',
            type: 'range',
            min: 0,
            max: 240,
            step: 1,
            autoZero: '不限制',
          },
          // 以下两项后置：新增参数排在既有字段之后，不打断原有阅读顺序。
          {
            key: 'videoEncoder',
            label: '--video-encoder',
            type: 'text',
            placeholder: 'OMX.qcom.video.encoder.avc',
            hint: '留空由 scrcpy 自动选择',
            detail: [
              '指定 MediaCodec 视频编码器（可用的编码器名随 --video-codec 的编码格式而定）。',
              '可用 scrcpy --list-encoders 列出设备上可用的编码器。',
            ],
          },
          {
            key: 'hwdec',
            label: '--hwdec',
            type: 'select',
            options: [
              // 「自动」= 不传该 flag（scrcpy 自身默认 auto：优先第一个可用的硬件解码器，
              // 无法解码时回退软件解码）。以下三项才是显式指定。
              { label: '自动', value: '' },
              { label: 'disabled', value: 'disabled' },
              { label: 'vaapi', value: 'vaapi' },
              { label: 'd3d11va', value: 'd3d11va' },
            ],
            hint: '「后台录制」开启时不生效',
            detail: [
              '「自动」= 不传该 flag：scrcpy 使用第一个可用的硬件解码器，无法解码时回退软件解码。',
              'disabled 强制纯软件解码；vaapi 仅 Linux（需系统安装 VA-API 驱动，X11 上还要求 EGL 渲染器）；d3d11va 仅 Windows。显式指定后若硬件解码不可用，scrcpy 直接报错退出。',
              '与「后台录制」（--no-window）互斥：该模式隐含 --no-video-playback，而硬件解码要求 video playback；disabled 不受此限。',
              '需要 scrcpy 5.0 及以上。',
            ],
          },
        ],
      },
      {
        title: '画面与适配',
        fields: [
          {
            key: 'crop',
            label: '--crop W:H:X:Y',
            type: 'text',
            placeholder: '1224:1440:0:0',
          },
          {
            key: 'angle',
            label: '--angle',
            type: 'segmented',
            options: SEG_ANGLE,
          },
          {
            key: 'renderFit',
            label: '--render-fit',
            type: 'select',
            options: [
              { label: 'letterbox', value: 'letterbox' },
              { label: 'stretched', value: 'stretched' },
              { label: 'unscaled', value: 'unscaled' },
            ],
          },
          {
            key: 'videoBuffer',
            label: '--video-buffer',
            type: 'number',
            min: 0,
            max: 60000,
            hint: '单位：毫秒',
          },
        ],
      },
    ],
  },
  {
    key: 'audio',
    title: '音频',
    subtitle: 'Android 11+ 可转发设备音频，支持多种音源与编码。',
    cards: [
      {
        title: '音频转发',
        fields: [
          {
            key: 'audioOff',
            label: '--no-audio',
            type: 'toggle',
            // 开关语义是「禁用音频转发」：开 ⇒ 下发 `--no-audio`；出厂即开（见 `defaults.ts`）。
            hint: '禁用音频转发',
          },
          {
            key: 'audioSource',
            label: '--audio-source',
            type: 'select',
            options: [
              { label: 'output', value: 'output' },
              { label: 'playback', value: 'playback' },
              { label: 'mic', value: 'mic' },
              { label: 'mic-unprocessed', value: 'mic-unprocessed' },
              { label: 'mic-camcorder', value: 'mic-camcorder' },
              { label: 'mic-voice-communication', value: 'mic-voice-communication' },
              { label: 'voice-call', value: 'voice-call' },
              { label: 'voice-performance', value: 'voice-performance' },
            ],
          },
          {
            key: 'audioCodec',
            label: '--audio-codec',
            type: 'select',
            options: [
              { label: 'opus', value: 'opus' },
              { label: 'aac', value: 'aac' },
              { label: 'flac', value: 'flac' },
              { label: 'raw', value: 'raw' },
            ],
          },
          {
            key: 'audioDup',
            label: '--audio-dup',
            type: 'toggle',
            hint: '边播边录',
          },
          // 后置：新增参数排在既有字段之后。
          {
            key: 'audioEncoder',
            label: '--audio-encoder',
            type: 'text',
            placeholder: 'c2.android.opus.encoder',
            hint: '留空由 scrcpy 自动选择',
            detail: [
              '指定 MediaCodec 音频编码器（可用的编码器名随 --audio-codec 的编码格式而定）。',
              '可用 scrcpy --list-encoders 列出设备上可用的编码器。',
            ],
          },
        ],
      },
      {
        title: '码率与缓冲',
        fields: [
          {
            key: 'audioBitrate',
            label: '--audio-bit-rate',
            type: 'range',
            min: 0,
            max: 512,
            step: 32,
            unit: 'Kbps',
            // 最左端即「自动」= 不传该 flag，码率由 scrcpy 自身默认决定；出厂默认即「自动」。
            autoZero: '自动',
          },
          {
            key: 'audioBuffer',
            label: '--audio-buffer',
            type: 'number',
            min: 0,
            max: 10000,
            hint: '单位：毫秒',
          },
          {
            key: 'audioOutBuffer',
            label: '--audio-output-buffer',
            type: 'number',
            min: 0,
            max: 10000,
            hint: '单位：毫秒',
          },
        ],
      },
    ],
  },
  {
    key: 'control',
    title: '控制',
    subtitle: '键鼠/手柄注入方式、快捷键、屏幕与剪贴板行为。',
    cards: [
      {
        title: '输入注入',
        fields: [
          {
            key: 'noControl',
            label: '--no-control',
            type: 'toggle',
            hint: '禁用设备控制',
          },
          {
            key: 'keyboard',
            label: '--keyboard',
            type: 'select',
            options: [
              { label: 'uhid', value: 'uhid' },
              { label: 'sdk', value: 'sdk' },
              { label: 'aoa', value: 'aoa' },
              { label: 'disabled', value: 'disabled' },
            ],
          },
          {
            key: 'mouse',
            label: '--mouse',
            type: 'select',
            options: [
              { label: 'uhid', value: 'uhid' },
              { label: 'sdk', value: 'sdk' },
              { label: 'aoa', value: 'aoa' },
              { label: 'disabled', value: 'disabled' },
            ],
          },
          {
            key: 'gamepad',
            label: '--gamepad',
            type: 'select',
            options: [
              { label: 'uhid', value: 'uhid' },
              { label: 'aoa', value: 'aoa' },
              { label: 'disabled', value: 'disabled' },
            ],
          },
          {
            key: 'shortMod',
            label: '--shortcut-mod',
            type: 'chips',
            options: [
              { label: 'lalt', value: 'lalt' },
              { label: 'lsuper', value: 'lsuper' },
              { label: 'lctrl', value: 'lctrl' },
              { label: 'rctrl', value: 'rctrl' },
              { label: 'ralt', value: 'ralt' },
              { label: 'rsuper', value: 'rsuper' },
            ],
          },
          // 后置：新增参数排在既有字段之后。
          {
            key: 'noKeyRepeat',
            label: '--no-key-repeat',
            type: 'toggle',
            hint: '按住按键时不转发重复事件',
          },
        ],
      },
      {
        title: '屏幕与剪贴板',
        fields: [
          {
            key: 'showTouches',
            label: '--show-touches',
            type: 'toggle',
            hint: '演示可见点击',
          },
          {
            key: 'turnScreenOff',
            label: '--turn-screen-off',
            type: 'toggle',
            // 与「视频源为摄像头」互斥：摄像头源下画面不来自设备屏幕，熄屏无处可谈。
            // 说明分两层（config ④）：常驻只留「熄灭手机屏」，互斥条件收进 `?` 详情。
            // 本定义是**唯一**的说明出口（参数页与设备覆盖面板共用同一份文字），拼装层同时把它剔除。
            hint: '镜像时熄灭手机屏',
            detail: ['视频源为摄像头时不生效：此时画面不来自设备屏幕，熄屏无从谈起。'],
          },
          {
            key: 'stayAwake',
            label: '--stay-awake',
            type: 'toggle',
            hint: '充电时保持亮屏',
          },
          {
            key: 'keepActive',
            label: '--keep-active',
            type: 'toggle',
            hint: '模拟活动防息屏',
          },
          {
            key: 'powerOffOnClose',
            label: '--power-off-on-close',
            type: 'toggle',
            hint: '退出即熄屏',
          },
          {
            key: 'clipboard',
            label: '--no-clipboard-autosync',
            type: 'toggle',
            hint: '双向同步（关闭时）',
          },
          // 后置：新增参数排在既有字段之后。
          {
            key: 'screenOffTimeout',
            label: '--screen-off-timeout',
            type: 'number',
            min: 0,
            max: 86400,
            hint: '单位：秒；0 / 留空 = 不修改设备设置',
            detail: ['scrcpy 运行期间把设备的息屏超时改为该秒数，退出时恢复原值。'],
          },
        ],
      },
    ],
  },
  {
    key: 'record',
    title: '录制',
    subtitle: '录制镜像到文件，或仅录制不显示窗口。',
    cards: [
      {
        title: '录制设置',
        fields: [
          {
            key: 'recordOn',
            label: '--record',
            type: 'toggle',
            hint: '保存到文件',
          },
          {
            key: 'recordPath',
            label: '文件路径',
            type: 'text',
            placeholder: 'D:/Recordings/session.mp4',
          },
          {
            key: 'recordFormat',
            label: '--record-format',
            type: 'select',
            options: [
              { label: 'mp4', value: 'mp4' },
              { label: 'mkv', value: 'mkv' },
              { label: 'm4a', value: 'm4a' },
              { label: 'mka', value: 'mka' },
              { label: 'opus', value: 'opus' },
              { label: 'aac', value: 'aac' },
              { label: 'flac', value: 'flac' },
              { label: 'wav', value: 'wav' },
            ],
          },
          {
            key: 'recordOrientation',
            label: '--record-orientation',
            type: 'segmented',
            options: SEG_ANGLE,
          },
        ],
      },
      {
        title: '无头与时限',
        fields: [
          {
            key: 'noWindow',
            label: '--no-window',
            type: 'toggle',
            hint: '后台录制',
          },
          {
            key: 'timeLimit',
            label: '--time-limit',
            type: 'number',
            min: 0,
            autoZero: '不限时',
            hint: '单位：秒；0 / 留空 = 不限时',
          },
          {
            key: 'pauseOnExit',
            label: '--pause-on-exit',
            type: 'select',
            options: [
              { label: 'false', value: 'false' },
              { label: 'true', value: 'true' },
              { label: 'if-error', value: 'if-error' },
            ],
          },
        ],
      },
    ],
  },
  {
    key: 'window',
    title: '窗口',
    subtitle: 'scrcpy 窗口的标题、尺寸、位置与外观。',
    cards: [
      {
        title: '外观',
        fields: [
          {
            key: 'windowTitle',
            label: '--window-title',
            type: 'text',
            hint: '留空 = 标题用设备名',
          },
          {
            key: 'borderless',
            label: '--window-borderless',
            type: 'toggle',
            hint: '隐藏装饰',
          },
          {
            key: 'fullscreen',
            label: '--fullscreen',
            type: 'toggle',
            hint: '启动即全屏',
          },
          {
            key: 'alwaysOnTop',
            label: '--always-on-top',
            type: 'toggle',
            hint: '最前显示',
          },
          {
            key: 'aspectLock',
            label: '--no-window-aspect-ratio-lock',
            type: 'toggle',
            hint: '保持比例（关闭时）',
          },
          { key: 'bgColor', label: '--background-color', type: 'text' },
        ],
      },
      {
        title: '尺寸与位置',
        fields: [
          {
            key: 'windowWidth',
            label: '--window-width',
            type: 'number',
            min: 0,
            max: 16384,
            hint: '0 / 留空 = 由 scrcpy 决定窗口大小',
          },
          {
            key: 'windowHeight',
            label: '--window-height',
            type: 'number',
            min: 0,
            max: 16384,
            hint: '0 / 留空 = 由 scrcpy 决定窗口大小',
          },
          {
            key: 'windowX',
            label: '--window-x',
            type: 'number',
            min: 0,
            max: 32767,
            hint: '0 / 留空 = 由 scrcpy 自动决定',
          },
          {
            key: 'windowY',
            label: '--window-y',
            type: 'number',
            min: 0,
            max: 32767,
            hint: '0 / 留空 = 由 scrcpy 自动决定',
          },
        ],
      },
    ],
  },
  {
    key: 'apps',
    // 「应用」是本组的归类标签，页名已改为「高级」（design D10：页名与分组标签解耦）。
    // 本组卡片由 AdvancedPage 渲染，该页还会带上 advanced 组的网络与自定义参数。
    title: '应用',
    subtitle:
      '连接后直接启动指定 App（虚拟显示场景尤其有用），用自定义参数兜底任何未覆盖的 flag，并可调整端口与隧道主机等网络参数。',
    cards: [
      {
        // 不取整行：它要与网络卡并排（design D11）。
        title: '启动应用',
        fields: [
          {
            key: 'startApp',
            label: '--start-app',
            type: 'text',
            placeholder: 'org.mozilla.firefox  /  ?firefox  /  +org.mozilla.firefox',
            hint: '支持 ? 模糊匹配 / + 强制停止',
          },
        ],
      },
    ],
  },
];

/**
 * 第 8 组「更多」的三张卡片（「scrcpy 运行环境」卡片不含默认参数字段）。
 *
 * 三张卡片各自具名、再以引用组装进分组：**两个页面**按名取用（自定义参数与网络 → 「高级」页，
 * 日志参数 → 「设置」页；本组已无独占页面），因此不是按下标 —— 下标在卡片增减时
 * 会**静默**指向相邻卡片（不报错，只渲染出错误的内容）。卡片标题与页标题不同名也是有意为之：
 * 页标题已占用「设置」，卡片再叫同一个名字只是重复。
 *
 * 会话输出不是参数（没有字段），不进本模型，由设置页的「会话输出」卡片自行渲染。
 */

/** 自定义参数透传（「高级」页）。 */
export const CUSTOM_ARGS_CARD: ParamCard = {
  title: '自定义参数（直接透传给 scrcpy）',
  span2: true,
  fields: [
    {
      key: 'customArgs',
      label: '额外参数',
      type: 'textarea',
      placeholder: '--keyboard=uhid --video-codec=h265',
      hint: '任何新版 scrcpy 的 flag 都可在此手动追加，UI 无需更新即可使用。会作用于所有未单独覆盖的设备',
    },
  ],
};

/** 日志相关的默认参数（设置页）。 */
export const RUN_LOG_CARD: ParamCard = {
  title: '运行日志参数',
  fields: [
    {
      key: 'verbosity',
      label: '--verbosity',
      type: 'select',
      options: [
        { label: 'info', value: 'info' },
        { label: 'debug', value: 'debug' },
        { label: 'verbose', value: 'verbose' },
        { label: 'warn', value: 'warn' },
        { label: 'error', value: 'error' },
      ],
    },
    { key: 'printFps', label: '--print-fps', type: 'toggle', hint: '控制台打印帧率' },
    {
      key: 'cleanup',
      label: '--no-cleanup',
      type: 'toggle',
      hint: '退出时还原设备状态（关闭时）',
    },
  ],
};

/** 端口与隧道主机（「高级」页）。 */
export const NETWORK_CARD: ParamCard = {
  title: '网络',
  fields: [
    {
      key: 'port',
      label: '--port',
      type: 'text',
      placeholder: '27183:27199',
      hint: '端口或「起始:结束」的端口范围',
    },
    {
      key: 'tunnelHost',
      label: '--tunnel-host',
      type: 'text',
      placeholder: 'localhost',
    },
  ],
};

/**
 * 第 8 组「更多」：与其余 7 组共用同一份字段模型，是为了让参数校验与页面渲染
 * 只有一个来源，每个 flag 也只在这里出现一次。
 *
 * 本组已**没有独占页面**：三张卡片分别落在「高级」页（自定义参数 / 网络）与
 * 设置页（日志参数）；末位入口「运行环境」页只承载运行环境与下载通道
 * （后者属 `channels`，不是默认参数）。因此组名「更多」与组内的 subtitle 只是模型内的
 * 归类标签，MUST NOT 当作页面名使用（页名与分组标签解耦，见 design D8 / D10）。
 * 字段归属（`defaults.advanced`）不变，故 `command.ts` 的拼装、`validate.ts` 的校验
 * 与设备覆盖都不受影响（design D1：只拆页、不拆组）。
 */
export const ADVANCED_GROUP: ParamGroup = {
  key: 'advanced',
  title: '高级',
  subtitle: '自定义参数兜底任何未覆盖的 flag，以及日志、诊断与网络端的运行参数。',
  cards: [CUSTOM_ARGS_CARD, RUN_LOG_CARD, NETWORK_CARD],
};

/** 8 组默认参数的完整清单（含「更多」组）—— 遍历与校验只走这里。 */
export const ALL_GROUPS: ParamGroup[] = [...PARAM_GROUPS, ADVANCED_GROUP];

/** 按分组 key 取分组定义。 */
export function groupOf(key: string): ParamGroup | undefined {
  return ALL_GROUPS.find((g) => g.key === key);
}

/**
 * 按「分组 + 字段」取字段定义：设备覆盖面板据此复用 8 组里的标签、量程与提示，
 * 不在面板里重写一套（否则同一 flag 的标签与取值范围会有两份，必然漂移）。
 */
export function fieldOf(groupKey: string, fieldKey: string): ParamField | undefined {
  return groupOf(groupKey)
    ?.cards.flatMap((c) => c.fields)
    .find((f) => f.key === fieldKey);
}

/** 某个字段在参数模型里的地址（`group.key`）。 */
export interface FieldRef {
  group: string;
  key: string;
}

/**
 * 设备页覆盖面板的「最常用项」（tasks 7.8）：
 * 分辨率 / 码率 / 帧率 / 窗口标题 / 关屏 / 只读 / 全屏 / 启动应用。
 *
 * 「启动应用」压在末位：面板后半段是「更多参数」提示行，新项插在它之前；
 * 该 flag 的标签与 placeholder 直接取自 apps 组，不在这里另写一份。
 */
export const OVERRIDE_FIELD_REFS: FieldRef[] = [
  { group: 'video', key: 'maxSize' },
  { group: 'video', key: 'bitrate' },
  { group: 'video', key: 'maxFps' },
  { group: 'window', key: 'windowTitle' },
  { group: 'control', key: 'turnScreenOff' },
  { group: 'control', key: 'noControl' },
  { group: 'window', key: 'fullscreen' },
  { group: 'apps', key: 'startApp' },
];

/** 字段地址的字符串形式（与校验错误的 key 一致）。 */
export function refKey(group: string, key: string): string {
  return `${group}.${key}`;
}
