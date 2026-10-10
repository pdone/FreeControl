// 参数输入校验（tasks 7.3）：数值范围 + 关键字段的格式。
//
// 校验规则贴着 8 组字段模型（`src/lib/params.ts`）走，量程取自字段自身的 `min` / `max`，
// 不在别处再抄一份 —— 参数的合法范围只有一处声明。错误 key 与字段地址一致（`group.key`），
// 因此「默认参数」「设备覆盖」「命令拼装」三处能共用同一份错误表。
//
// 语义（spec「参数输入校验」）：非法值给出明确提示，且 **不用于发起会话** ——
// 命令拼装会跳过非法字段（见 `command.ts::buildArgs` 的 `invalid` 参数）。

import { ALL_GROUPS, refKey, type ParamField } from './params';

/** 字段校验错误：key 为 `group.key`，值为可直接展示在界面上的提示文案。 */
export type ParamErrors = Record<string, string>;

const RES_RE = /^\d+x\d+(\/\d+)?$/;
const SIZE_RE = /^\d+x\d+$/;
const CROP_RE = /^\d+:\d+:\d+:\d+$/;
const HEX_COLOR_RE = /^#[0-9a-fA-F]{6}$/;
const PORT_RE = /^\d+(:\d+)?$/;

/** 未填写（空串 / null / undefined）视为「未设置」，不参与拼装，也就不算非法。 */
export function isBlank(v: unknown): boolean {
  return (
    v === null || v === undefined || (typeof v === 'string' && v.trim().length === 0)
  );
}

function asNumber(v: unknown): number | null {
  if (isBlank(v)) return null;
  const n = typeof v === 'number' ? v : Number(String(v).trim());
  return Number.isFinite(n) ? n : null;
}

/** `--port`：单个端口或「起始:结束」范围，两端都在 1–65535 且起始不大于结束。 */
function isPortSpec(v: string): boolean {
  if (!PORT_RE.test(v)) return false;
  const [from, to] = v.split(':');
  const a = Number(from);
  const b = to === undefined ? a : Number(to);
  if (a < 1 || a > 65535 || b < 1 || b > 65535) return false;
  return a <= b;
}

/** 引号 / 括号是否配对（自定义参数是按空白切分后原样透传的，未配对会让后续参数被吞掉）。 */
function quotesBalanced(v: string): boolean {
  const single = (v.match(/'/g) ?? []).length;
  const double = (v.match(/"/g) ?? []).length;
  return single % 2 === 0 && double % 2 === 0;
}

/** 文本类字段的格式规则：只校验有明确格式约定的几项，其余（标题 / 路径 / 包名）不设限。 */
function validateText(field: ParamField, raw: string): string | null {
  const v = raw.trim();
  if (v === '') return null;
  switch (field.key) {
    case 'crop': {
      if (!CROP_RE.test(v)) return `${field.label} 需为 W:H:X:Y 四个整数`;
      const [w, h] = v.split(':');
      return Number(w) > 0 && Number(h) > 0 ? null : `${field.label} 的宽高需大于 0`;
    }
    case 'newDisplayRes':
      return RES_RE.test(v)
        ? null
        : `${field.label} 需为「宽x高」或「宽x高/DPI」，例如 1920x1080/420`;
    case 'cameraSize':
      return SIZE_RE.test(v) ? null : `${field.label} 需为「宽x高」，例如 1920x1080`;
    case 'bgColor':
      return HEX_COLOR_RE.test(v) ? null : `${field.label} 需为 #RRGGBB 形式`;
    case 'port':
      return isPortSpec(v)
        ? null
        : `${field.label} 需为端口或「起始:结束」，两端在 1–65535 之间`;
    case 'tunnelHost':
      return /\s/.test(v) ? `${field.label} 不能包含空格` : null;
    case 'customArgs':
      return quotesBalanced(v) ? null : '自定义参数中的引号未配对';
    default:
      return null;
  }
}

/** 单字段校验：合法返回 null，非法返回提示文案（tasks 7.3）。 */
export function validateField(field: ParamField, value: unknown): string | null {
  if (field.type === 'number' || field.type === 'range') {
    if (isBlank(value)) return null;
    const n = asNumber(value);
    if (n === null) return `${field.label} 需为数字`;
    if (field.min !== undefined && n < field.min) return `${field.label} 不得小于 ${field.min}`;
    if (field.max !== undefined && n > field.max) return `${field.label} 不得大于 ${field.max}`;
    return null;
  }
  if (field.type === 'text' || field.type === 'textarea') {
    const v = typeof value === 'string' ? value : value === null || value === undefined ? '' : String(value);
    return validateText(field, v);
  }
  // toggle / segmented / select / chips：取值由界面给定，不可能格式非法。
  return null;
}

/** 一个分组内的取值校验（分组取值形如 `{ maxSize: 1080, ... }`）。 */
export function validateGroupValues(
  groupKey: string,
  values: Record<string, unknown> | null | undefined,
): ParamErrors {
  const out: ParamErrors = {};
  if (!values) return out;
  const group = ALL_GROUPS.find((g) => g.key === groupKey);
  if (!group) return out;
  for (const card of group.cards) {
    for (const f of card.fields) {
      if (!(f.key in values)) continue;
      const err = validateField(f, values[f.key]);
      if (err) out[refKey(groupKey, f.key)] = err;
    }
  }
  return out;
}

/**
 * 校验一份完整的默认参数（8 组）。
 * 只校验字段模型里存在的项：配置里残留的未知键不参与拼装，也就不该报错。
 */
export function validateParams(groups: Record<string, Record<string, unknown>> | null | undefined): ParamErrors {
  const out: ParamErrors = {};
  for (const g of ALL_GROUPS) {
    Object.assign(out, validateGroupValues(g.key, groups?.[g.key]));
  }
  return out;
}

/** 校验一份设备覆盖取值（`overrides[id].values`，形如 `{ video: { maxFps: 30 } }`）。 */
export function validateOverrideValues(
  values: Record<string, unknown> | null | undefined,
): ParamErrors {
  const out: ParamErrors = {};
  if (!values) return out;
  for (const g of ALL_GROUPS) {
    const group = values[g.key];
    if (group && typeof group === 'object') {
      Object.assign(out, validateGroupValues(g.key, group as Record<string, unknown>));
    }
  }
  return out;
}

/** 错误表 → 字段地址集合，供命令拼装剔除非法值（`command.ts::buildArgs`）。 */
export function invalidKeys(errors: ParamErrors): Set<string> {
  return new Set(Object.keys(errors));
}
