import type { DeviceListSnapshot } from './types';

/**
 * 设备域快照的修订号门控。
 *
 * 修订号只由后端在设备记录 / 排序偏好 / 最近启动时间变化时递增；相同或更低的修订号
 * 一律视为迟到快照（同修订号的内容按契约应相同）。首次接收（`current === null`）放行，
 * 以便接纳旧配置里缺省的 revision 0。
 */
export function reduceDeviceList(
  current: DeviceListSnapshot | null,
  incoming: DeviceListSnapshot,
): { next: DeviceListSnapshot; accepted: boolean } {
  if (current && incoming.deviceList.revision <= current.deviceList.revision) {
    return { next: current, accepted: false };
  }
  return { next: incoming, accepted: true };
}

/**
 * 运行态快照序号门控（与设备域修订号独立）。
 *
 * 连接状态变化不落盘，用独立的递增序号防止「主动查询的旧结果晚于新事件到达」把
 * 已连接回退成在线。
 */
export function acceptStatusSequence(current: number, incoming: number): boolean {
  return incoming > current;
}
