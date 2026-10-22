// 8 组默认参数的数据驱动描述（对照 ui-design/launcher-prototype.html 与 doc/scrcpy）。
// 表单控件据此渲染；flag 名称均来自 scrcpy 源码。前端拥有字段模型（design D5/D7）。
//
// 文案分工：人读标签 / 提示 / 详情走 `MessageKey`；flag 名、取值名（如 `uhid` / `h264`）
// 与单位保持原文，直接放在 `label` 里。组件用 `labelKey ?? label` 解析。

import type { MessageKey } from './i18n/zh';

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
  /** 技术取值名（如 `uhid` / `h264`）保持原文；人读标签用 `labelKey`。 */
  label?: string;
  labelKey?: MessageKey;
  value: string;
}

export interface ParamField {
  key: string;
  /** flag 名等原文标签；人读标签用 `labelKey`。 */
  label?: string;
  labelKey?: MessageKey;
  type: FieldType;
  options?: FieldOption[];
  min?: number;
  max?: number;
  step?: number;
  /** 数值单位，如 Mbps / Kbps / px。 */
  unit?: string;
  /** 当数值为 0（或空）时显示的替代文案，如「自动」「不限制」。 */
  autoZeroKey?: MessageKey;
  placeholder?: string;
  hintKey?: MessageKey;
  /**
   * 详情段落：较长、或带前置条件 / 平台限制的说明按语义拆段，收进标签右侧 `?` 的详情面板
   * （`HelpTip.svelte`，展开全由 CSS 承担）；字段下方只留一句短 `hint`。
   * 分配口径见 config.yaml ④：常驻短句与详情之间的分配可变，但信息不得丢。
   */
  detailKeys?: MessageKey[];
  /** hint 中以警示色呈现的片段（如可读性不足）。 */
  hintWarnKey?: MessageKey;
  /**
   * 字段级显隐：入参是该字段**所在组**的取值（与卡片级 `ParamCard.showWhen` 同源）。
   * 用于「取值在这个状态下不参与拼装，于是干脆不显示它」的字段 —— 显示着一个不生效的
   * 控件，比把它藏起来更容易让人以为设置丢了（如视频源为摄像头时的「显示模式」）。
   */
  showWhen?: (g: Record<string, unknown>) => boolean;
}

export interface ParamCard {
  titleKey: MessageKey;
  span2?: boolean;
  showWhen?: (g: Record<string, unknown>) => boolean;
  fields: ParamField[];
}

export interface ParamGroup {
  key: string;
  titleKey: MessageKey;
  subtitleKey: MessageKey;
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
    titleKey: 'param.source.title',
    subtitleKey:
      'param.source.subtitle',
    cards: [
      {
        titleKey: 'param.source.origin.title',
        fields: [
          {
            key: 'videoSource',
            label: '--video-source',
            type: 'segmented',
            options: [
              { labelKey: 'param.source.videoSource.opt.display', value: 'display' },
              { labelKey: 'param.source.videoSource.opt.camera', value: 'camera' },
            ],
            detailKeys: [
              'param.source.videoSource.detail1',
              'param.source.videoSource.detail2',
            ],
          },
          {
            key: 'displayMode',
            labelKey: 'param.source.displayMode.label',
            type: 'segmented',
            options: [
              { labelKey: 'param.source.displayMode.opt.main', value: 'main' },
              { labelKey: 'param.source.displayMode.opt.new', value: 'new' },
              { label: 'Flex Display', value: 'flex' },
            ],
            // 视频源为摄像头时这一档不参与拼装（camera 分支优先），显示出来只会让人以为
            // 设置生效了 —— 与原型 `toggleSourceUI()` 的行为一致。
            showWhen: (g) => g.videoSource !== 'camera',
            hintKey: 'param.source.displayMode.hint',
            detailKeys: [
              'param.source.displayMode.detail1',
              'param.source.displayMode.detail2',
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
            hintKey: 'param.source.displayId.hint',
            detailKeys: [
              'param.source.displayId.detail1',
              'param.source.displayId.detail2',
            ],
          },
        ],
      },
      {
        titleKey: 'param.source.virtual.title',
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
            hintKey: 'param.source.newDisplayRes.hint',
            detailKeys: [
              'param.source.newDisplayRes.detail1',
              'param.source.newDisplayRes.detail2',
            ],
          },
          {
            key: 'newDisplayDpi',
            labelKey: 'param.source.newDisplayDpi.label',
            type: 'number',
            min: 72,
            max: 1000,
            hintKey: 'param.source.newDisplayDpi.hint',
            detailKeys: [
              'param.source.newDisplayDpi.detail1',
              'param.source.newDisplayDpi.detail2',
            ],
          },
          {
            key: 'noVdDestroy',
            label: '--no-vd-destroy-content',
            type: 'toggle',
            hintKey: 'param.source.noVdDestroy.hint',
            detailKeys: [
              'param.source.noVdDestroy.detail1',
              'param.source.noVdDestroy.detail2',
            ],
          },
        ],
      },
      {
        titleKey: 'param.source.camera.title',
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
              { labelKey: 'common.auto', value: '' },
              { label: 'front', value: 'front' },
              { label: 'back', value: 'back' },
              { label: 'external', value: 'external' },
            ],
            detailKeys: [
              'param.source.cameraFacing.detail1',
              'param.source.cameraFacing.detail2',
            ],
          },
          {
            key: 'cameraSize',
            label: '--camera-size',
            type: 'text',
            placeholder: '1920x1080',
            hintKey: 'param.source.cameraSize.hint',
            detailKeys: [
              'param.source.cameraSize.detail1',
              'param.source.cameraSize.detail2',
            ],
          },
          {
            key: 'cameraFps',
            label: '--camera-fps',
            type: 'number',
            min: 1,
            max: 240,
            placeholder: '30',
            hintKey: 'param.source.cameraFps.hint',
            detailKeys: [
              'param.source.cameraFps.detail1',
              'param.source.cameraFps.detail2',
            ],
          },
          {
            key: 'cameraZoom',
            label: '--camera-zoom',
            type: 'number',
            min: 0.1,
            max: 10,
            step: 0.1,
            detailKeys: [
              'param.source.cameraZoom.detail1',
              'param.source.cameraZoom.detail2',
            ],
          },
          {
            key: 'cameraTorch',
            label: '--camera-torch',
            type: 'toggle',
            hintKey: 'param.source.cameraTorch.hint',
            detailKeys: ['param.source.cameraTorch.detail1', 'param.source.cameraTorch.detail2'],
          },
        ],
      },
    ],
  },
  {
    key: 'video',
    titleKey: 'param.video.title',
    subtitleKey: 'param.video.subtitle',
    cards: [
      {
        titleKey: 'param.video.codec.title',
        fields: [
          {
            key: 'videoCodec',
            label: '--video-codec',
            type: 'select',
            options: [
              // 「自动」= 不传该 flag，编码由 scrcpy 自身默认决定（见 change `defaults-tuning`），
              // 且出厂默认即「自动」。空串沿用既有的「未设置」语义，拼装层无需为它加分支。
              { labelKey: 'common.auto', value: '' },
              { label: 'h264', value: 'h264' },
              { label: 'h265', value: 'h265' },
              { label: 'av1', value: 'av1' },
              { label: 'vp8', value: 'vp8' },
              { label: 'vp9', value: 'vp9' },
            ],
            detailKeys: [
              'param.video.videoCodec.detail1',
              'param.video.videoCodec.detail2',
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
            autoZeroKey: 'common.auto',
            detailKeys: [
              'param.video.maxSize.detail1',
              'param.video.maxSize.detail2',
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
            autoZeroKey: 'common.auto',
            detailKeys: [
              'param.video.bitrate.detail1',
              'param.video.bitrate.detail2',
            ],
          },
          {
            key: 'maxFps',
            label: '--max-fps',
            type: 'range',
            min: 0,
            max: 240,
            step: 1,
            autoZeroKey: 'common.unlimited',
            detailKeys: [
              'param.video.maxFps.detail1',
              'param.video.maxFps.detail2',
            ],
          },
          // 以下两项后置：新增参数排在既有字段之后，不打断原有阅读顺序。
          {
            key: 'videoEncoder',
            label: '--video-encoder',
            type: 'text',
            placeholder: 'OMX.qcom.video.encoder.avc',
            hintKey: 'param.video.videoEncoder.hint',
            detailKeys: [
              'param.video.videoEncoder.detail1',
              'param.video.videoEncoder.detail2',
            ],
          },
          {
            key: 'hwdec',
            label: '--hwdec',
            type: 'select',
            options: [
              // 「自动」= 不传该 flag（scrcpy 自身默认 auto：优先第一个可用的硬件解码器，
              // 无法解码时回退软件解码）。以下三项才是显式指定。
              { labelKey: 'common.auto', value: '' },
              { label: 'disabled', value: 'disabled' },
              { label: 'vaapi', value: 'vaapi' },
              { label: 'd3d11va', value: 'd3d11va' },
            ],
            hintKey: 'param.video.hwdec.hint',
            detailKeys: [
              'param.video.hwdec.detail1',
              'param.video.hwdec.detail2',
              'param.video.hwdec.detail3',
              'param.video.hwdec.detail4',
            ],
          },
        ],
      },
      {
        titleKey: 'param.video.fit.title',
        fields: [
          {
            key: 'crop',
            label: '--crop W:H:X:Y',
            type: 'text',
            placeholder: '1224:1440:0:0',
            detailKeys: [
              'param.video.crop.detail1',
              'param.video.crop.detail2',
              'param.video.crop.detail3',
            ],
          },
          {
            key: 'angle',
            label: '--angle',
            type: 'segmented',
            options: SEG_ANGLE,
            detailKeys: [
              'param.video.angle.detail1',
              'param.video.angle.detail2',
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
            detailKeys: [
              'param.video.renderFit.detail1',
              'param.video.renderFit.detail2',
            ],
          },
          {
            key: 'videoBuffer',
            label: '--video-buffer',
            type: 'number',
            min: 0,
            max: 60000,
            hintKey: 'param.video.videoBuffer.hint',
            detailKeys: [
              'param.video.videoBuffer.detail1',
              'param.video.videoBuffer.detail2',
            ],
          },
        ],
      },
    ],
  },
  {
    key: 'audio',
    titleKey: 'param.audio.title',
    subtitleKey: 'param.audio.subtitle',
    cards: [
      {
        titleKey: 'param.audio.forward.title',
        fields: [
          {
            key: 'audioOff',
            label: '--no-audio',
            type: 'toggle',
            // 开关语义是「禁用音频转发」：开 ⇒ 下发 `--no-audio`；出厂即开（见 `defaults.ts`）。
            hintKey: 'param.audio.audioOff.hint',
            detailKeys: [
              'param.audio.audioOff.detail1',
              'param.audio.audioOff.detail2',
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
            detailKeys: [
              'param.audio.audioSource.detail1',
              'param.audio.audioSource.detail2',
              'param.audio.audioSource.detail3',
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
            detailKeys: [
              'param.audio.audioCodec.detail1',
            ],
          },
          {
            key: 'audioDup',
            label: '--audio-dup',
            type: 'toggle',
            hintKey: 'param.audio.audioDup.hint',
            detailKeys: [
              'param.audio.audioDup.detail1',
              'param.audio.audioDup.detail2',
            ],
          },
          // 后置：新增参数排在既有字段之后。
          {
            key: 'audioEncoder',
            label: '--audio-encoder',
            type: 'text',
            placeholder: 'c2.android.opus.encoder',
            hintKey: 'param.audio.audioEncoder.hint',
            detailKeys: [
              'param.audio.audioEncoder.detail1',
              'param.audio.audioEncoder.detail2',
            ],
          },
        ],
      },
      {
        titleKey: 'param.audio.rate.title',
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
            autoZeroKey: 'common.auto',
            detailKeys: [
              'param.audio.audioBitrate.detail1',
              'param.audio.audioBitrate.detail2',
            ],
          },
          {
            key: 'audioBuffer',
            label: '--audio-buffer',
            type: 'number',
            min: 0,
            max: 10000,
            hintKey: 'param.audio.audioBuffer.hint',
            detailKeys: [
              'param.audio.audioBuffer.detail1',
              'param.audio.audioBuffer.detail2',
            ],
          },
          {
            key: 'audioOutBuffer',
            label: '--audio-output-buffer',
            type: 'number',
            min: 0,
            max: 10000,
            hintKey: 'param.audio.audioOutBuffer.hint',
            detailKeys: [
              'param.audio.audioOutBuffer.detail1',
              'param.audio.audioOutBuffer.detail2',
            ],
          },
        ],
      },
    ],
  },
  {
    key: 'control',
    titleKey: 'param.control.title',
    subtitleKey: 'param.control.subtitle',
    cards: [
      {
        titleKey: 'param.control.input.title',
        fields: [
          {
            key: 'noControl',
            label: '--no-control',
            type: 'toggle',
            hintKey: 'param.control.noControl.hint',
            detailKeys: [
              'param.control.noControl.detail1',
              'param.control.noControl.detail2',
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
            detailKeys: [
              'param.control.keyboard.detail1',
              'param.control.keyboard.detail2',
              'param.control.keyboard.detail3',
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
            detailKeys: [
              'param.control.mouse.detail1',
              'param.control.mouse.detail2',
              'param.control.mouse.detail3',
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
            detailKeys: [
              'param.control.gamepad.detail1',
              'param.control.gamepad.detail2',
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
            detailKeys: [
              'param.control.shortMod.detail1',
              'param.control.shortMod.detail2',
            ],
          },
          // 后置：新增参数排在既有字段之后。
          {
            key: 'noKeyRepeat',
            label: '--no-key-repeat',
            type: 'toggle',
            hintKey: 'param.control.noKeyRepeat.hint',
            detailKeys: [
              'param.control.noKeyRepeat.detail1',
              'param.control.noKeyRepeat.detail2',
            ],
          },
        ],
      },
      {
        titleKey: 'param.control.screen.title',
        fields: [
          {
            key: 'showTouches',
            label: '--show-touches',
            type: 'toggle',
            hintKey: 'param.control.showTouches.hint',
            detailKeys: [
              'param.control.showTouches.detail1',
              'param.control.showTouches.detail2',
            ],
          },
          {
            key: 'turnScreenOff',
            label: '--turn-screen-off',
            type: 'toggle',
            // 与「视频源为摄像头」互斥：摄像头源下画面不来自设备屏幕，熄屏无处可谈。
            // 说明分两层（config ④）：常驻只留「熄灭手机屏」，互斥条件收进 `?` 详情。
            // 本定义是**唯一**的说明出口（参数页与设备覆盖面板共用同一份文字），拼装层同时把它剔除。
            hintKey: 'param.control.turnScreenOff.hint',
            detailKeys: [
              'param.control.turnScreenOff.detail1',
              'param.control.turnScreenOff.detail2',
            ],
          },
          {
            key: 'stayAwake',
            label: '--stay-awake',
            type: 'toggle',
            hintKey: 'param.control.stayAwake.hint',
            detailKeys: [
              'param.control.stayAwake.detail1',
              'param.control.stayAwake.detail2',
            ],
          },
          {
            key: 'keepActive',
            label: '--keep-active',
            type: 'toggle',
            hintKey: 'param.control.keepActive.hint',
            detailKeys: [
              'param.control.keepActive.detail1',
              'param.control.keepActive.detail2',
            ],
          },
          {
            key: 'powerOffOnClose',
            label: '--power-off-on-close',
            type: 'toggle',
            hintKey: 'param.control.powerOffOnClose.hint',
            detailKeys: ['param.control.powerOffOnClose.detail1'],
          },
          {
            key: 'clipboard',
            label: '--no-clipboard-autosync',
            type: 'toggle',
            hintKey: 'param.control.clipboard.hint',
            detailKeys: [
              'param.control.clipboard.detail1',
              'param.control.clipboard.detail2',
            ],
          },
          // 后置：新增参数排在既有字段之后。
          {
            key: 'screenOffTimeout',
            label: '--screen-off-timeout',
            type: 'number',
            min: 0,
            max: 86400,
            hintKey: 'param.control.screenOffTimeout.hint',
            detailKeys: ['param.control.screenOffTimeout.detail1'],
          },
        ],
      },
    ],
  },
  {
    key: 'record',
    titleKey: 'param.record.title',
    subtitleKey: 'param.record.subtitle',
    cards: [
      {
        titleKey: 'param.record.settings.title',
        fields: [
          {
            key: 'recordOn',
            label: '--record',
            type: 'toggle',
            hintKey: 'param.record.recordOn.hint',
            detailKeys: [
              'param.record.recordOn.detail1',
              'param.record.recordOn.detail2',
            ],
          },
          {
            key: 'recordPath',
            labelKey: 'param.record.recordPath.label',
            type: 'text',
            placeholder: 'D:/Recordings/session.mp4',
            detailKeys: [
              'param.record.recordPath.detail1',
              'param.record.recordPath.detail2',
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
            detailKeys: [
              'param.record.recordFormat.detail1',
              'param.record.recordFormat.detail2',
            ],
          },
          {
            key: 'recordOrientation',
            label: '--record-orientation',
            type: 'segmented',
            options: SEG_ANGLE,
            detailKeys: [
              'param.record.recordOrientation.detail1',
              'param.record.recordOrientation.detail2',
            ],
          },
        ],
      },
      {
        titleKey: 'param.record.headless.title',
        fields: [
          {
            key: 'noWindow',
            label: '--no-window',
            type: 'toggle',
            hintKey: 'param.record.noWindow.hint',
            detailKeys: [
              'param.record.noWindow.detail1',
              'param.record.noWindow.detail2',
            ],
          },
          {
            key: 'timeLimit',
            label: '--time-limit',
            type: 'number',
            min: 0,
            autoZeroKey: 'common.noTimeLimit',
            hintKey: 'param.record.timeLimit.hint',
            detailKeys: ['param.record.timeLimit.detail1'],
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
            detailKeys: [
              'param.record.pauseOnExit.detail1',
              'param.record.pauseOnExit.detail2',
            ],
          },
        ],
      },
    ],
  },
  {
    key: 'window',
    titleKey: 'param.window.title',
    subtitleKey: 'param.window.subtitle',
    cards: [
      {
        titleKey: 'param.window.appearance.title',
        fields: [
          {
            key: 'windowTitle',
            label: '--window-title',
            type: 'text',
            hintKey: 'param.window.windowTitle.hint',
            detailKeys: ['param.window.windowTitle.detail1'],
          },
          {
            key: 'borderless',
            label: '--window-borderless',
            type: 'toggle',
            hintKey: 'param.window.borderless.hint',
            detailKeys: [
              'param.window.borderless.detail1',
              'param.window.borderless.detail2',
            ],
          },
          {
            key: 'fullscreen',
            label: '--fullscreen',
            type: 'toggle',
            hintKey: 'param.window.fullscreen.hint',
            detailKeys: ['param.window.fullscreen.detail1'],
          },
          {
            key: 'alwaysOnTop',
            label: '--always-on-top',
            type: 'toggle',
            hintKey: 'param.window.alwaysOnTop.hint',
            detailKeys: ['param.window.alwaysOnTop.detail1'],
          },
          {
            key: 'aspectLock',
            label: '--no-window-aspect-ratio-lock',
            type: 'toggle',
            hintKey: 'param.window.aspectLock.hint',
            detailKeys: [
              'param.window.aspectLock.detail1',
              'param.window.aspectLock.detail2',
            ],
          },
          {
            key: 'bgColor',
            label: '--background-color',
            type: 'text',
            detailKeys: [
              'param.window.bgColor.detail1',
              'param.window.bgColor.detail2',
            ],
          },
        ],
      },
      {
        titleKey: 'param.window.geometry.title',
        fields: [
          {
            key: 'windowWidth',
            label: '--window-width',
            type: 'number',
            min: 0,
            max: 16384,
            hintKey: 'param.window.windowWidth.hint',
            detailKeys: [
              'param.window.windowWidth.detail1',
              'param.window.windowWidth.detail2',
            ],
          },
          {
            key: 'windowHeight',
            label: '--window-height',
            type: 'number',
            min: 0,
            max: 16384,
            hintKey: 'param.window.windowHeight.hint',
            detailKeys: [
              'param.window.windowHeight.detail1',
              'param.window.windowHeight.detail2',
            ],
          },
          {
            key: 'windowX',
            label: '--window-x',
            type: 'number',
            min: 0,
            max: 32767,
            hintKey: 'param.window.windowX.hint',
            detailKeys: [
              'param.window.windowX.detail1',
              'param.window.windowX.detail2',
            ],
          },
          {
            key: 'windowY',
            label: '--window-y',
            type: 'number',
            min: 0,
            max: 32767,
            hintKey: 'param.window.windowY.hint',
            detailKeys: [
              'param.window.windowY.detail1',
              'param.window.windowY.detail2',
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
    titleKey: 'param.apps.title',
    subtitleKey:
      'param.apps.subtitle',
    cards: [
      {
        // 不取整行：它要与网络卡并排（design D11）。
        titleKey: 'param.apps.launch.title',
        fields: [
          {
            key: 'startApp',
            label: '--start-app',
            type: 'text',
            placeholder: 'org.mozilla.firefox  /  ?firefox  /  +org.mozilla.firefox',
            hintKey: 'param.apps.startApp.hint',
            detailKeys: [
              'param.apps.startApp.detail1',
              'param.apps.startApp.detail2',
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
  titleKey: 'param.advanced.custom.title',
  span2: true,
  fields: [
    {
      key: 'customArgs',
      labelKey: 'param.advanced.customArgs.label',
      type: 'textarea',
      placeholder: '--keyboard=uhid --video-codec=h265',
      hintKey: 'param.advanced.customArgs.hint',
      detailKeys: [
        'param.advanced.customArgs.detail1',
        'param.advanced.customArgs.detail2',
        'param.advanced.customArgs.detail3',
      ],
    },
  ],
};

/** 日志相关的默认参数（设置页）。 */
export const RUN_LOG_CARD: ParamCard = {
  titleKey: 'param.advanced.runLog.title',
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
      detailKeys: [
        'param.advanced.verbosity.detail1',
        'param.advanced.verbosity.detail2',
      ],
    },
    {
      key: 'printFps',
      label: '--print-fps',
      type: 'toggle',
      hintKey: 'param.advanced.printFps.hint',
      detailKeys: ['param.advanced.printFps.detail1'],
    },
    {
      key: 'cleanup',
      label: '--no-cleanup',
      type: 'toggle',
      hintKey: 'param.advanced.cleanup.hint',
      detailKeys: [
        'param.advanced.cleanup.detail1',
        'param.advanced.cleanup.detail2',
      ],
    },
  ],
};

/** 端口与隧道主机（「高级」页）。 */
export const NETWORK_CARD: ParamCard = {
  titleKey: 'param.advanced.network.title',
  fields: [
    {
      key: 'port',
      label: '--port',
      type: 'text',
      placeholder: '27183:27199',
      hintKey: 'param.advanced.port.hint',
      detailKeys: [
        'param.advanced.port.detail1',
        'param.advanced.port.detail2',
      ],
    },
    {
      key: 'tunnelHost',
      label: '--tunnel-host',
      type: 'text',
      placeholder: 'localhost',
      detailKeys: [
        'param.advanced.tunnelHost.detail1',
        'param.advanced.tunnelHost.detail2',
        'param.advanced.tunnelHost.detail3',
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
  titleKey: 'param.advanced.title',
  subtitleKey: 'param.advanced.subtitle',
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
