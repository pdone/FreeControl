import type { MessageKey, MessageParams } from './i18n';
import type { ChannelView } from './types';

type Translate = (key: MessageKey, params?: MessageParams) => string;

/** 下载通道的当前语言标签；官方通道无地址，代理通道带完整地址。 */
export function channelLabel(
  channel: ChannelView | null | undefined,
  t: Translate,
): string {
  if (!channel) return '';
  return channel.kind === 'official'
    ? t('channel.official')
    : t('channel.proxy', { url: channel.url });
}
