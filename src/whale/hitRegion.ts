// src/whale/hitRegion.ts —— 命中区上报：轮询鲸鱼娘根元素窗口坐标，变化超阈值时同步到 Rust 命中测试层
import { invoke } from '@tauri-apps/api/core';
import type { HitRect } from './types';

/** 矩形变化阈值（CSS 像素）：低于此变化不重新上报，避免高频 invoke。 */
const THRESHOLD_PX = 4;

function hasTauriRuntime(): boolean {
  return typeof window !== 'undefined' && ('__TAURI_INTERNALS__' in window || '__TAURI__' in window);
}

/** 判断两个矩形是否足够接近（各边位移均小于阈值）。 */
function closeEnough(a: HitRect, b: HitRect, threshold: number): boolean {
  return (
    Math.abs(a.x - b.x) < threshold &&
    Math.abs(a.y - b.y) < threshold &&
    Math.abs(a.w - b.w) < threshold &&
    Math.abs(a.h - b.h) < threshold
  );
}

/**
 * 命中区上报器：独立 rAF 轮询根元素的 `getBoundingClientRect()`，
 * 只在矩形变化超过阈值时 `invoke('set_hit_region', { rect })`。
 * 注意：getBoundingClientRect 返回 CSS 像素（窗口内逻辑坐标），与 Rust 侧一致。
 */
export class HitRegionReporter {
  private root: HTMLElement | null = null;
  private raf: number | null = null;
  private last: HitRect | null = null;
  private running = false;

  start(root: HTMLElement): void {
    if (!hasTauriRuntime()) return; // 纯 vite dev 无 Tauri，跳过
    this.stop(false); // 防重复 start：仅取消旧 rAF，保留 last 以便下次立即上报
    this.root = root;
    this.running = true;
    this.raf = requestAnimationFrame(this.tick);
  }

  stop(clear = true): void {
    this.running = false;
    if (this.raf !== null) {
      cancelAnimationFrame(this.raf);
      this.raf = null;
    }
    if (clear) {
      this.root = null;
      this.last = null;
      if (hasTauriRuntime()) {
        // 销毁/隐藏时清空命中区，让整窗穿透
        void invoke('set_hit_region', { rect: null }).catch(() => undefined);
      }
    }
  }

  private tick = (): void => {
    if (!this.running) return;
    this.report();
    this.raf = requestAnimationFrame(this.tick);
  };

  private report(): void {
    if (!this.root || !hasTauriRuntime()) return;
    const r = this.root.getBoundingClientRect();
    const rect: HitRect = { x: r.left, y: r.top, w: r.width, h: r.height };
    if (this.last && closeEnough(rect, this.last, THRESHOLD_PX)) return;
    this.last = rect;
    void invoke('set_hit_region', { rect }).catch(() => undefined);
  }
}
