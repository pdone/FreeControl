import type { MessageKey, MessageParams } from './i18n';
import { fieldOf } from './params';
import type { ValidationMessage } from './validate';

type Translate = (key: MessageKey, params?: MessageParams) => string;

/** 字段 / 选项的显示标签：人读文案用 `labelKey`，flag 名与技术取值用原文字段 `label`。 */
export function labelText(
  item: { label?: string; labelKey?: MessageKey },
  t: Translate,
): string {
  return item.labelKey ? t(item.labelKey) : item.label ?? '';
}

/** 校验错误 → 当前语言文本；字段名由错误里的 `fieldRef` 回查字段模型。 */
export function validationText(error: ValidationMessage, t: Translate): string {
  const dot = error.fieldRef.indexOf('.');
  const group = dot >= 0 ? error.fieldRef.slice(0, dot) : error.fieldRef;
  const key = dot >= 0 ? error.fieldRef.slice(dot + 1) : '';
  const field = fieldOf(group, key);
  const fieldLabel = field ? labelText(field, t) : error.fieldRef;
  return t(error.code, { field: fieldLabel, ...error.params });
}
