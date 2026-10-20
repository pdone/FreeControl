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
            detail: [
              '镜像手机屏幕，或改为采集设备摄像头的画面。',
              '摄像头镜像需要 Android 12 及以上；摄像头源下画面不来自设备屏幕，--display-id、--new-display 与 --turn-screen-off 等参数不参与拼装。',
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
            hint: '两档虚拟屏的分辨率都在下方指定',
            detail: [
              '主屏镜像设备当前的显示；虚拟显示器在设备上新开一块独立显示；Flex Display 让虚拟屏随窗口连续缩放。',
              'Flex Display 下 --crop、--window-width、--window-height 与 --no-control 不参与拼装（scrcpy 判定它们与 --flex-display 不兼容）。',
            ],
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
            detail: [
              '虚拟显示器的分辨率，格式为「宽x高」。',
              '它与下方的密度一起拼成 --new-display=宽x高/密度：只填密度时分辨率沿用主屏尺寸。',
            ],
          },
          {
            key: 'newDisplayDpi',
            label: '密度 DPI',
            type: 'number',
            min: 72,
            max: 1000,
            hint: '留空用上面的默认密度',
            detail: [
              '虚拟显示器的像素密度，与分辨率一起决定新屏上界面元素的大小。',
              '同一分辨率下密度越大，界面元素越大、可容纳的内容越少。',
            ],
          },
          {
            key: 'noVdDestroy',
            label: '--no-vd-destroy-content',
            type: 'toggle',
            hint: '移到主屏而非销毁',
            detail: [
              '关闭虚拟显示器时，把其中运行的应用移到主屏，而不是随虚拟屏一起销毁。',
              '只在虚拟显示器与 Flex Display 下有意义。',
            ],
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
            detail: [
              '按朝向选择设备摄像头：front 前置、back 后置、external 外接。',
              '不选则由 scrcpy 使用检测到的第一个摄像头。',
            ],
          },
          {
            key: 'cameraSize',
            label: '--camera-size',
            type: 'text',
            placeholder: '1920x1080',
            hint: '留空由 scrcpy 自动选择尺寸',
            detail: [
              '摄像头的采集分辨率，格式为「宽x高」；可选档位由摄像头自身决定，可用 scrcpy --list-camera-sizes 列出。',
              '摄像头镜像需要 Android 12 及以上。',
            ],
          },
          {
            key: 'cameraFps',
            label: '--camera-fps',
            type: 'number',
            min: 1,
            max: 240,
            placeholder: '30',
            hint: '留空用 Android 默认的 30 fps',
            detail: [
              '摄像头的采集帧率，可选档位同样取决于摄像头。',
              '摄像头镜像需要 Android 12 及以上。',
            ],
          },
          {
            key: 'cameraZoom',
            label: '--camera-zoom',
            type: 'number',
            min: 0.1,
            max: 10,
            step: 0.1,
            detail: [
              '摄像头的初始缩放倍数，1 表示不缩放。',
              '可调范围由摄像头自身决定，填超出范围的倍数会由 scrcpy 报错。',
            ],
          },
          {
            key: 'cameraTorch',
            label: '--camera-torch',
            type: 'toggle',
            hint: '启动时点亮',
            detail: ['启动时点亮摄像头的闪光灯（手电筒）。', '只在摄像头源下有效。'],
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
            detail: [
              '选择视频编码格式。h264 是 scrcpy 的默认值，延迟最低；h265 画质更好但延迟更高；av1 在现有 Android 设备上的编码器还很罕见。',
              '选「自动」时按 scrcpy 自身的默认决定。',
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
            detail: [
              '限制画面宽与高的最大值，另一边按设备比例推算（Flex Display 除外）。',
              '摄像头源下它不裁剪画面，而是用于从摄像头支持的尺寸里挑接近的一档。',
            ],
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
            detail: [
              '视频编码的比特率，scrcpy 默认 8 Mbps。',
              '越高画面越清晰，占用的带宽与解码资源也越多。',
            ],
          },
          {
            key: 'maxFps',
            label: '--max-fps',
            type: 'range',
            min: 0,
            max: 240,
            step: 1,
            autoZero: '不限制',
            detail: [
              '限制采集帧率，Android 10 起获得官方支持。',
              '实际帧率随画面变化：静止画面不会跑满上限。',
            ],
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
            detail: [
              '在设备端裁剪画面，四个分量的顺序是「宽:高:x:y」，其中 x、y 是裁剪区域左上角的坐标。',
              '坐标以设备自然方向为基准（手机竖放、平板横放）。',
              'Flex Display 下不生效。',
            ],
          },
          {
            key: 'angle',
            label: '--angle',
            type: 'segmented',
            options: SEG_ANGLE,
            detail: [
              '按给定角度顺时针旋转画面内容，可填任意角度，界面给出 0 / 90 / 180 / 270 四档。',
              '只改变画面呈现，不影响录制文件的方向（录制方向由 --record-orientation 单独决定）。',
            ],
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
            detail: [
              '控制画面在窗口里的适配方式。',
              'letterbox 保持宽高比，多出的区域留边；stretched 拉满窗口、不保持宽高比；unscaled 按像素一比一显示，窗口小于画面时只能看到一部分。',
            ],
          },
          {
            key: 'videoBuffer',
            label: '--video-buffer',
            type: 'number',
            min: 0,
            max: 60000,
            hint: '单位：毫秒',
            detail: [
              '在显示前增加一段缓冲，用来吸收网络抖动。',
              '它以减少卡顿为代价换来画面延迟增加；局域网直连通常不需要。',
            ],
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
            detail: [
              '开关打开时不转发设备音频：画面照常镜像，声音仍从手机发出。',
              '关掉开关才启用音频转发；音频转发需要 Android 11 及以上。',
            ],
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
            detail: [
              '选择音频采集来源，只在音频转发开启时才有意义。',
              'output 转发整机音频输出并让设备端静音；playback 只采集应用的播放声音（应用可主动退出采集）；mic 系列采集麦克风 —— mic-unprocessed 取原始信号、mic-camcorder 按录像场景调优、mic-voice-communication 启用回声消除与自动增益；voice-call 与 voice-performance 分别对应通话与演出场景。',
              '设备实际支持的来源随厂商实现而异。',
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
            detail: [
              '选择音频编码格式：opus 为 scrcpy 默认，aac 兼容性好，flac 无损但体积大，raw 不压缩、体积最大。',
            ],
          },
          {
            key: 'audioDup',
            label: '--audio-dup',
            type: 'toggle',
            hint: '边播边录',
            detail: [
              '转发音频的同时让设备继续外放，而不是转发后设备静音。',
              '只在音频源为 playback 时可用。',
            ],
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
            detail: [
              '音频编码的比特率，scrcpy 默认 128 Kbps。',
              '只影响转发到电脑的音频，不影响设备自身的播放。',
            ],
          },
          {
            key: 'audioBuffer',
            label: '--audio-buffer',
            type: 'number',
            min: 0,
            max: 10000,
            hint: '单位：毫秒',
            detail: [
              '音频缓冲的时长，用来吸收网络抖动。',
              '调小会降低延迟，但更容易出现欠载、听感上是断音或爆音。',
            ],
          },
          {
            key: 'audioOutBuffer',
            label: '--audio-output-buffer',
            type: 'number',
            min: 0,
            max: 10000,
            hint: '单位：毫秒',
            detail: [
              '电脑端音频输出缓冲的大小，scrcpy 默认 10 毫秒。',
              '除非确有需要不要改动：改小更容易出现播放断续。',
            ],
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
            detail: [
              '只镜像、不注入：鼠标与键盘事件都不发给设备，等于只读查看。',
              'Flex Display 下该参数不参与拼装（scrcpy 判定不兼容）。',
            ],
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
            detail: [
              '选择键盘事件的注入方式。',
              'disabled 不向设备发送键盘事件；sdk 通过 Android 系统接口把事件投给应用；uhid 把电脑键盘模拟成设备上的物理键盘；aoa 用 AOAv2 协议模拟物理键盘，仅 USB 连接可用。',
              'uhid 与 aoa 需要先在设备的「设置 → 系统 → 语言和输入法 → 物理键盘」里配好键盘布局。',
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
            detail: [
              '选择鼠标事件的注入方式。',
              'disabled 不向设备发送鼠标事件；sdk 通过 Android 系统接口投递；uhid 模拟物理 HID 鼠标；aoa 用 AOAv2 协议模拟，仅 USB 连接可用。',
              'uhid 与 aoa 会捕获电脑鼠标（相对移动模式），按 LAlt、LSuper 或 RSuper 可把鼠标交还给电脑。',
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
            detail: [
              '选择手柄的注入方式。',
              'disabled 不向设备发送手柄事件；uhid 把电脑手柄模拟成设备上的物理手柄；aoa 用 AOAv2 协议模拟，仅 USB 连接可用。',
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
            detail: [
              '指定 scrcpy 快捷键的修饰键，默认是左 Alt 或左 Super。',
              '可同时选择多个，任一命中即触发；改动后所有快捷键的组合随之变化。',
            ],
          },
          // 后置：新增参数排在既有字段之后。
          {
            key: 'noKeyRepeat',
            label: '--no-key-repeat',
            type: 'toggle',
            hint: '按住按键时不转发重复事件',
            detail: [
              '按住按键时不向设备转发重复事件。',
              '适合长按交给设备端自身连发的场景（如长按方向键连续翻页）。',
            ],
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
            detail: [
              '启动时打开设备的「显示点按操作反馈」，退出时恢复原值。',
              '只显示设备上的物理触摸，scrcpy 自己的鼠标点击不会显示。',
            ],
          },
          {
            key: 'turnScreenOff',
            label: '--turn-screen-off',
            type: 'toggle',
            // 与「视频源为摄像头」互斥：摄像头源下画面不来自设备屏幕，熄屏无处可谈。
            // 说明分两层（config ④）：常驻只留「熄灭手机屏」，互斥条件收进 `?` 详情。
            // 本定义是**唯一**的说明出口（参数页与设备覆盖面板共用同一份文字），拼装层同时把它剔除。
            hint: '镜像时熄灭手机屏',
            detail: [
              '启动后立即关闭设备屏幕，画面仍照常镜像，设备端的触控与按键照常可用。',
              '视频源为摄像头时不生效：此时画面不来自设备屏幕，熄屏无从谈起。',
            ],
          },
          {
            key: 'stayAwake',
            label: '--stay-awake',
            type: 'toggle',
            hint: '充电时保持亮屏',
            detail: [
              '设备处于充电状态时保持屏幕常亮，退出镜像时恢复原值。',
              '不充电时不生效。',
            ],
          },
          {
            key: 'keepActive',
            label: '--keep-active',
            type: 'toggle',
            hint: '模拟活动防息屏',
            detail: [
              '通过模拟用户活动保持屏幕常亮，不依赖充电状态。',
              '与「充电时保持亮屏」的区别：这一项看的是有没有输入，而不是有没有插电。',
            ],
          },
          {
            key: 'powerOffOnClose',
            label: '--power-off-on-close',
            type: 'toggle',
            hint: '退出即熄屏',
            detail: ['退出镜像时关闭设备屏幕，等价于按一次电源键。'],
          },
          {
            key: 'clipboard',
            label: '--no-clipboard-autosync',
            type: 'toggle',
            hint: '双向同步（关闭时）',
            detail: [
              '开启时剪贴板双向自动同步：向设备注入 Ctrl+V 之前先把电脑剪贴板推过去，设备剪贴板变化时同步回电脑。',
              '关闭后不再自动同步，两侧剪贴板互不影响。',
            ],
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
            detail: [
              '把镜像过程中的画面与声音录到文件，路径、格式与方向在下方设置。',
              '不勾选「后台录制」时窗口照常显示，录制与显示互不排斥。',
            ],
          },
          {
            key: 'recordPath',
            label: '文件路径',
            type: 'text',
            placeholder: 'D:/Recordings/session.mp4',
            detail: [
              '录制文件的保存路径，仅在启用录制时生效。',
              '扩展名决定容器格式（如 .mp4 / .mkv），与「格式」不一致时会得到名不副实的文件。',
            ],
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
            detail: [
              '强制录制文件的容器格式，仅在启用录制时生效。',
              '默认 mp4；与文件扩展名不一致时，文件内容与后缀会对不上。',
            ],
          },
          {
            key: 'recordOrientation',
            label: '--record-orientation',
            type: 'segmented',
            options: SEG_ANGLE,
            detail: [
              '只影响录制文件的方向，不改变镜像画面。',
              '90 与 270 会交换输出文件的宽高。',
            ],
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
            detail: [
              '不打开 scrcpy 窗口，只在后台运行；控制仍可用，但看不到画面。',
              '没有同时开启录制时，这个会话将在完全看不到输出的情况下运行。',
            ],
          },
          {
            key: 'timeLimit',
            label: '--time-limit',
            type: 'number',
            min: 0,
            autoZero: '不限时',
            hint: '单位：秒；0 / 留空 = 不限时',
            detail: ['到时限后自动结束会话，常用在按固定时长录制时。'],
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
            detail: [
              '会话结束后暂停等待，而不是立即退出；选 if-error 时只在出错后暂停。',
              '它的原意是让命令行窗口停住，以便读报错；本应用会把 scrcpy 的输出收进会话日志，因此通常用不到。',
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
            detail: ['自定义 scrcpy 窗口的标题，多设备同时投屏时可用来区分窗口。'],
          },
          {
            key: 'borderless',
            label: '--window-borderless',
            type: 'toggle',
            hint: '隐藏装饰',
            detail: [
              '去掉窗口边框与标题栏，画面铺满整个窗口。',
              '没有标题栏后可拖动的区域消失，位置与大小需借助快捷键或启动参数调整。',
            ],
          },
          {
            key: 'fullscreen',
            label: '--fullscreen',
            type: 'toggle',
            hint: '启动即全屏',
            detail: ['启动即进入全屏；退出全屏用 scrcpy 的全屏快捷键。'],
          },
          {
            key: 'alwaysOnTop',
            label: '--always-on-top',
            type: 'toggle',
            hint: '最前显示',
            detail: ['窗口始终压在其他窗口之上，适合边看手机边操作电脑。'],
          },
          {
            key: 'aspectLock',
            label: '--no-window-aspect-ratio-lock',
            type: 'toggle',
            hint: '保持比例（关闭时）',
            detail: [
              'scrcpy 默认锁定窗口宽高比；打开这个开关后可以自由拖动窗口大小，画面随之拉伸。',
              '只在使用 scrcpy 自带窗口时生效。',
            ],
          },
          {
            key: 'bgColor',
            label: '--background-color',
            type: 'text',
            detail: [
              '窗口留边区域（letterbox 黑边）的颜色，取值形如 #RGB 或 #RRGGBB。',
              'scrcpy 默认为 #222（深灰）。',
            ],
          },
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
            detail: [
              'scrcpy 窗口的初始宽度（像素）。',
              'Flex Display 下窗口尺寸由 --new-display 决定，这一项不参与拼装。',
            ],
          },
          {
            key: 'windowHeight',
            label: '--window-height',
            type: 'number',
            min: 0,
            max: 16384,
            hint: '0 / 留空 = 由 scrcpy 决定窗口大小',
            detail: [
              'scrcpy 窗口的初始高度（像素）。',
              'Flex Display 下窗口尺寸由 --new-display 决定，这一项不参与拼装。',
            ],
          },
          {
            key: 'windowX',
            label: '--window-x',
            type: 'number',
            min: 0,
            max: 32767,
            hint: '0 / 留空 = 由 scrcpy 自动决定',
            detail: [
              'scrcpy 窗口初始位置的横坐标（屏幕像素）。',
              '填了它（或 Y）的设备不再参与窗口自动排列，窗口落在指定位置。',
            ],
          },
          {
            key: 'windowY',
            label: '--window-y',
            type: 'number',
            min: 0,
            max: 32767,
            hint: '0 / 留空 = 由 scrcpy 自动决定',
            detail: [
              'scrcpy 窗口初始位置的纵坐标（屏幕像素）。',
              '填了它（或 X）的设备不再参与窗口自动排列，窗口落在指定位置。',
            ],
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
            detail: [
              '连接后直接在设备上打开指定应用；虚拟显示场景尤其有用，应用会开在新屏上。',
              '按名称匹配时 scrcpy 需要先读取设备上的应用列表，第一次可能稍慢。',
            ],
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
      hint: '对所有设备生效，直接追加到命令末尾',
      detail: [
        '把填写的内容原样追加到命令末尾，不做拆分与改写；界面没有提供的新 flag 也能这样用。',
        '它不属于设备覆盖：无论该设备是否启用覆盖，都会带上这段参数。',
        '引号必须成对，否则校验会拦下并给出提示。',
      ],
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
      detail: [
        '设置 scrcpy 的日志级别，决定会话日志里能看到多详细的过程信息。',
        '发布版 scrcpy 默认 info；调到 debug 或 verbose 会显著放大日志量。',
      ],
    },
    {
      key: 'printFps',
      label: '--print-fps',
      type: 'toggle',
      hint: '控制台打印帧率',
      detail: ['在日志里持续打印实际帧率，用来判断画面是否掉帧。'],
    },
    {
      key: 'cleanup',
      label: '--no-cleanup',
      type: 'toggle',
      hint: '退出时还原设备状态（关闭时）',
      detail: [
        'scrcpy 退出时会删除设备上的服务端程序，并还原它改动过的状态（显示触摸点、保持唤醒与电源模式）。',
        '打开这个开关即跳过清理：设备会保留这些改动，仅在排查问题时使用。',
      ],
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
      detail: [
        'scrcpy 客户端监听并接收视频流的 TCP 端口，scrcpy 默认 27183:27199。',
        '同时连多台设备时会在范围内依次取用。',
      ],
    },
    {
      key: 'tunnelHost',
      label: '--tunnel-host',
      type: 'text',
      placeholder: 'localhost',
      detail: [
        '指定 adb 隧道要连到的目标主机，默认 localhost。',
        '指定它之后 scrcpy 会自动改用 --force-adb-forward（不再尝试 adb reverse）。',
        '只有当设备端能访问另一台主机时才需要改。',
      ],
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
