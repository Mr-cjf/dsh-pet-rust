// src/whale/videoManager.ts —— 双缓冲 A/B video 播放管理
import type { AnimKey } from './types';
import { MANIFEST, pickAnimationFile } from './videoManifest';
import { el, style } from './dom';
import { invoke } from '@tauri-apps/api/core';

type Facing = 'left' | 'right';

const FACING_CLASS = 'is-front';

/** 切换代数防过期的 pending 记录 */
interface Pending {
  key: AnimKey;
  once: boolean;
  gen: number;
}

export class VideoManager {
  private readonly frontA: HTMLVideoElement;
  private readonly frontB: HTMLVideoElement;
  private frontIndex: 0 | 1 = 0; // 当前显示层：0=A, 1=B
  private gen = 0; // 每次切换 +1，回调校验防过期覆盖
  private pending: Pending | null = null;
  private current: { key: AnimKey; once: boolean } | null = null;
  private facing: Facing = 'right';
  onEnded: (() => void) | null = null;

  constructor(stage: HTMLElement) {
    this.frontA = this.makeVideo();
    this.frontB = this.makeVideo();
    this.frontA.classList.add(FACING_CLASS);
    style(stage, { overflow: 'hidden' }); // 保持 mount 中设置的 absolute+inset:0，避免塌陷裁剪
    stage.appendChild(this.frontA);
    stage.appendChild(this.frontB);
  }

  private makeVideo(): HTMLVideoElement {
    const v = el('video', 'whale-video');
    v.muted = true;
    v.playsInline = true;
    v.preload = 'auto';
    v.setAttribute('draggable', 'false');
    return v;
  }

  private facingTransform(): string {
    return this.facing === 'left' ? 'scaleX(-1)' : '';
  }

  setFacing(facing: Facing): void {
    this.facing = facing;
    const t = this.facingTransform();
    this.frontA.style.transform = t;
    this.frontB.style.transform = t;
  }

  getFacing(): Facing {
    return this.facing;
  }

  isPlaying(key: AnimKey, once: boolean): boolean {
    return !!this.current && this.current.key === key && this.current.once === once;
  }

  /** 播放动画；once=true 播一次触发 onEnded，false 循环；force=true 强制重载（皮肤切换用） */
  play(key: AnimKey, once: boolean, force = false): void {
    if (!force && this.isPlaying(key, once)) return; // 已在播同样动画，去重
    const entry = MANIFEST[key];
    const nextGen = ++this.gen;
    this.pending = { key, once, gen: nextGen };

    // 目标 = 当前非显示层
    const target = this.frontIndex === 0 ? this.frontB : this.frontA;
    const old = this.frontIndex === 0 ? this.frontA : this.frontB;
    const file = pickAnimationFile(key);
    target.src = file;
    target.loop = once ? false : entry.loop;
    target.onended = once ? this.makeEnded(nextGen) : null;
    target.currentTime = 0;
    target.load();

    const onReady = () => {
      target.removeEventListener('loadeddata', onReady);
      if (this.pending?.gen !== nextGen) return; // 过期回调放弃
      target.classList.add(FACING_CLASS);
      if (old !== target) old.classList.remove(FACING_CLASS);
      this.frontIndex = this.frontIndex === 0 ? 1 : 0;
      this.pending = null;
      this.current = { key, once };
      target.style.transform = this.facingTransform();
      target.play().catch(() => {});
      void invoke('frontend_log', { msg: `video ready: ${key} ${file} w=${target.videoWidth} h=${target.videoHeight}` }).catch(() => undefined);
    };
    target.addEventListener('loadeddata', onReady);

    const onError = () => {
      target.removeEventListener('error', onError);
      if (this.pending?.gen !== nextGen) return; // 过期回调放弃
      console.error('[videoManager] 视频加载失败:', file);
      void invoke('frontend_log', { msg: `video ERROR: ${key} ${file}` }).catch(() => undefined);
      this.pending = null;
    };
    target.addEventListener('error', onError);

    if (target.readyState >= 2) onReady();
  }

  private makeEnded(gen: number): () => void {
    return () => {
      if (this.gen !== gen) return; // 过期 ended 忽略
      const cb = this.onEnded;
      if (cb) cb();
    };
  }

  stop(): void {
    this.gen++;
    this.pending = null;
    this.current = null;
    for (const v of [this.frontA, this.frontB]) {
      v.onended = null;
      v.pause();
      v.removeAttribute('src');
      v.load();
    }
  }

  /** 移动驱动用：当前前台视频播放进度（秒） */
  currentTime(): number {
    const v = this.frontIndex === 0 ? this.frontA : this.frontB;
    return Number.isFinite(v.currentTime) ? v.currentTime : 0;
  }

  duration(): number {
    const v = this.frontIndex === 0 ? this.frontA : this.frontB;
    return Number.isFinite(v.duration) && v.duration > 0 ? v.duration : 0;
  }

  destroy(): void {
    this.stop();
    this.frontA.remove();
    this.frontB.remove();
  }
}
