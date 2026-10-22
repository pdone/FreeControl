import { translateCode, type MessageKey, type MessageParams } from './i18n';
import type { UserError, UserMessage } from './types';

export type Translate = (key: MessageKey, params?: MessageParams) => string;

function isUserMessage(value: unknown): value is UserMessage {
  return (
    typeof value === 'object' &&
    value !== null &&
    typeof (value as { code?: unknown }).code === 'string'
  );
}

function bodyText(msg: UserMessage | UserError, t: Translate): string {
  const detail = 'detail' in msg ? msg.detail : undefined;
  const translated = t(msg.code as MessageKey, msg.params);
  if (translated !== msg.code) {
    return detail ? `${translated}${t('common.detailSeparator')}${detail}` : translated;
  }
  return detail ?? msg.code;
}

/**
 * 后端消息 → 当前语言文本。
 *
 * 字典尚未收录该 code 时先回退 `detail`（技术原文），再回退 code 本身；
 * 第 3 组补齐后端错误码字典后，同一调用会自动出正式文案。
 */
export function messageText(msg: UserMessage | UserError | null | undefined): string {
  if (!msg) return '';
  return bodyText(msg, (key, params) => translateCode(key, params));
}

/**
 * 异常对象 / 字符串 → 当前语言文本。传进来的 `t` 让调用方模板直接建立语言依赖，
 * 已显示的失败原因因此会随语言切换重译。
 */
export function errorTextOf(e: unknown, t: Translate): string {
  if (typeof e === 'string') return e;
  if (e instanceof Error) return e.message;
  if (isUserMessage(e)) return bodyText(e, t);
  return t('common.unknownError');
}

/** 命令失败 → 可直接展示的一行文本；兼容旧字符串与 Error。 */
export function errorText(e: unknown): string {
  return errorTextOf(e, (key, params) => translateCode(key, params));
}
