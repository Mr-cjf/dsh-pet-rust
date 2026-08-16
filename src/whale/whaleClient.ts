// src/whale/whaleClient.ts —— 核心交互：链式状态机 + 双缓冲视频 + 拖拽/点击/移动
import type { AnimKey, DshSnapshot } from './types';
import { append, el, style } from './dom';
import { VideoManager } from './videoManager';
import { HitRegionReporter } from './hitRegion';
import { Bubble } from './bubble';
import { mapDshState } from './stateMapper';

type Facing = 'left' | 'right';

// 常量（对齐参考 client.js）
const SIZE = 260;          // 显示尺寸 px
const CANVAS_H = 360;      // thumb 画布高度
const FEET_Y = 330;        // 画布中"脚底"y 坐标
const MOVE_MIN_PX = 60;
const MOVE_MAX_PX = 240;
const MOVE_MARGIN = 20;    // 屏幕边缘安全边距
const MOVE_LEAD_SEC = 2;   // 移动动画开头准备动作时长
const MOVE_TAIL_SEC = 2;   // 移动动画结尾收尾动作时长
const DRAG_THRESHOLD = 5;  // 拖拽判定阈值 px
const GHOST_CLICK_MS = 100;

// 动画池（键全部来自 videoManifest）
const IDLE_POOL: readonly AnimKey[] = [
  'idle_lookaround', 'idle_hum', 'idle_stretch', 'idle_yawn', 'idle_squash',
];
const ACT_POOL: readonly AnimKey[] = [
  'work_cube', 'work_toycar', 'work_tap', 'act_violin', 'act_watergun',
  'act_jump', 'act_spin', 'act_caught', 'act_rage', 'act_tailslap',
  'fx_bubbles', 'react_bow', 'react_scared',
];
const MOVE_POOL: readonly AnimKey[] = ['move_crab', 'move_floatstep', 'move_runleft'];
const CLICK_POOL: readonly AnimKey[] = ['click_happy', 'click_shy', 'click_tsundere'];
const WORK_POOL: readonly AnimKey[] = ['work_cube', 'work_toycar', 'work_tap'];
const TURN_KEY: AnimKey = 'idle_lookaround';
const IDLE_BASE: AnimKey = 'idle_breath';
const DRAG_KEY: AnimKey = 'drag_hang';

function pick<T>(pool: readonly T[], exclude?: T): T {
  const entries = exclude === undefined ? pool : pool.filter((x) => x !== exclude);
  return entries[Math.floor(Math.random() * entries.length)];
}

function randomBetween(min: number, max: number): number {
  return Math.floor(min + Math.random() * (max - min));
}

type Phase = 'idle' | 'turn' | 'idle_once' | 'move' | 'work' | 'react' | 'user' | 'offline';

interface DragState {
  active: boolean;
  dragging: boolean;
  sx: number;
  sy: number;
}

interface MovePlan {
  startRatio: number;
  targetRatio: number;
  startYRatio: number;
  dir: 1 | -1;
  totalRatio: number;
}

export class WhaleClient {
  private container: HTMLElement | null = null;
  private root: HTMLElement | null = null;
  private stage: HTMLElement | null = null;
  private bubble = new Bubble();
  private video: VideoManager | null = null;
  private hitRegion = new HitRegionReporter();

  private facing: Facing = 'right';
  private pos: { rx: number; ry: number } | null = null;
  private baseState = 'idle';
  private phase: Phase = 'idle';
  private curKey: AnimKey = IDLE_BASE;

  private idleTimer: number | null = null;
  private drag: DragState = { active: false, dragging: false, sx: 0, sy: 0 };
  private justDragged = false;
  private moveRaf: number | null = null;
  private moveToken = 0;
  private movePlan: MovePlan | null = null;

  private onWindowResize = (): void => this.renderPos();

  mount(container: HTMLElement): void {
    this.container = container;
    this.root = el('div', 'whale-root');
    this.stage = el('div', 'whale-stage');
    style(this.root, {
      width: SIZE + 'px',
      height: SIZE + 'px',
      position: 'absolute',
      willChange: 'left, top',
    });
    style(this.stage, { position: 'absolute', inset: '0', pointerEvents: 'auto' });

    this.bubble.mount(this.root);
    append(this.root, this.stage);
    append(container, this.root);

    this.video = new VideoManager(this.stage);
    this.video.onEnded = () => this.handleEnded();
    this.video.setFacing(this.facing);
    this.applyLanding();

    this.stage.addEventListener('pointerdown', (e) => this.onPointerDown(e));
    window.addEventListener('pointermove', (e) => this.onPointerMove(e));
    window.addEventListener('pointerup', (e) => this.onPointerUp(e));
    window.addEventListener('pointercancel', (e) => this.onPointerUp(e));
    window.addEventListener('resize', this.onWindowResize);

    // 默认右下角地面
    this.pos = { rx: 1 - (24 + SIZE / 2) / window.innerWidth, ry: 1 - (20 + SIZE / 2) / window.innerHeight };
    this.renderPos();
    this.hitRegion.start(this.root);
    this.startIdle();
  }

  setState(snapshot: DshSnapshot): void {
    const mapped = mapDshState(snapshot);
    const changed = snapshot.state !== this.baseState;

    if (changed) {
      this.stopIdle();
      this.stopMove();
      this.baseState = snapshot.state;
      this.container?.setAttribute('data-mode', snapshot.state);
    }

    // 气泡（idle 无气泡；offline 气泡单独显示）
    if (mapped.bubbleText) this.bubble.show(mapped.bubbleText);
    else if (mapped.kind !== 'offline') this.bubble.hide();

    if (!changed) return; // 同状态快照：只刷新气泡

    switch (mapped.kind) {
      case 'idle':
        this.startIdle();
        break;
      case 'work':
        this.startWork();
        break;
      case 'react':
        if (mapped.animKey) this.playOnce(mapped.animKey, 'react');
        break;
      case 'done':
        if (mapped.animKey) this.playOnce(mapped.animKey, 'react');
        break;
      case 'offline':
      default:
        this.enterOffline();
        break;
    }
  }

  // ---------- 本地交互优先级更高的动作触发（调试/外部） ----------
  actAction(key: AnimKey): void {
    this.playOnce(key, 'user');
  }

  turnAction(): void {
    this.playOnce(TURN_KEY, 'turn');
  }

  destroy(): void {
    this.stopIdle();
    this.stopMove();
    window.removeEventListener('resize', this.onWindowResize);
    window.removeEventListener('pointermove', this.onPointerMove);
    window.removeEventListener('pointerup', this.onPointerUp);
    window.removeEventListener('pointercancel', this.onPointerUp);
    this.video?.destroy();
    this.video = null;
    this.bubble.unmount();
    this.hitRegion.stop();
    if (this.root?.parentNode) this.root.parentNode.removeChild(this.root);
    this.root = null;
    this.stage = null;
    this.container = null;
  }

  // ---------- 播放原语 ----------
  private playLoop(key: AnimKey): void {
    this.curKey = key;
    this.video?.play(key, false);
  }

  private playOnce(key: AnimKey, phase: Phase): void {
    this.curKey = key;
    this.phase = phase;
    this.video?.play(key, true);
  }

  // ---------- 落地对齐 + 位置渲染 ----------
  private applyLanding(): void {
    if (!this.stage) return;
    const pad = (SIZE * (CANVAS_H - FEET_Y)) / CANVAS_H;
    this.stage.style.transform = 'translateY(' + pad + 'px)';
  }

  private clearLanding(): void {
    if (this.stage) this.stage.style.transform = 'none';
  }

  private renderPos(): void {
    if (!this.root || !this.pos) return;
    const W = window.innerWidth;
    const H = window.innerHeight;
    const half = SIZE / 2;
    const left = Math.min(Math.max(this.pos.rx * W - half, 0), W - SIZE);
    const top = Math.min(Math.max(this.pos.ry * H - half, 0), H - SIZE);
    this.root.style.left = left + 'px';
    this.root.style.top = top + 'px';
  }

  // ---------- 待机链（idle：idle_breath 循环 + 随机穿插） ----------
  private startIdle(): void {
    this.phase = 'idle';
    this.playLoop(IDLE_BASE);
    this.scheduleIdleAction();
  }

  /** 离线态：鲸鱼娘保持可见，继续播待机呼吸（灰化由 data-mode + CSS 处理） */
  private enterOffline(): void {
    this.phase = 'offline';
    this.playLoop(IDLE_BASE);
  }

  private scheduleIdleAction(): void {
    this.stopIdle();
    if (this.baseState !== 'idle' || this.phase !== 'idle') return;
    this.idleTimer = window.setTimeout(() => this.pickNext(), 3000 + Math.random() * 5000);
  }

  private stopIdle(): void {
    if (this.idleTimer !== null) {
      clearTimeout(this.idleTimer);
      this.idleTimer = null;
    }
  }

  private pickNext(): void {
    this.idleTimer = null;
    if (this.baseState !== 'idle' || this.phase !== 'idle') return;
    const roll = Math.random();
    if (roll < 0.3) {
      // 30% 继续待机（保持呼吸，重新武装随机器）
      this.scheduleIdleAction();
    } else if (roll < 0.4) {
      // 10% 转向
      this.playOnce(TURN_KEY, 'turn');
    } else if (roll < 0.8) {
      // 40% 随机动作
      this.playOnce(pick(IDLE_POOL.concat(ACT_POOL as AnimKey[]), this.curKey), 'idle_once');
    } else {
      // 20% 移动（空间不足回退动作）
      if (!this.tryMove()) {
        this.playOnce(pick(ACT_POOL, this.curKey), 'idle_once');
      }
    }
  }

  // ---------- 工作循环 ----------
  private startWork(): void {
    this.phase = 'work';
    this.playOnce(pick(WORK_POOL), 'work');
  }

  private nextWork(): void {
    if (this.baseState !== 'working') {
      this.resumeBase();
      return;
    }
    this.phase = 'work';
    this.playOnce(pick(WORK_POOL), 'work');
  }

  // ---------- 动画播完链 ----------
  private handleEnded(): void {
    if (this.drag.active) return; // 拖拽中不响应
    if (this.phase === 'turn') {
      this.setFacing(this.facing === 'right' ? 'left' : 'right');
    }
    switch (this.phase) {
      case 'turn':
      case 'idle_once':
      case 'move':
        this.resumeBase();
        break;
      case 'work':
        this.nextWork();
        break;
      case 'react':
        this.resumeBase();
        break;
      case 'user':
        this.resumeBase();
        break;
      case 'idle':
      case 'offline':
      default:
        break;
    }
  }

  private resumeBase(): void {
    if (this.baseState === 'working') this.startWork();
    else if (this.baseState === 'idle') this.startIdle();
    else if (this.baseState === 'offline') this.enterOffline();
    else {
      // attention / done 等瞬态基态：单发动画播完后回到待机呼吸循环，
      // 保持鲸鱼娘可见，等待下一个 dsh-state 更新（而不是 video.stop() 消失）。
      this.phase = 'idle';
      this.playLoop(IDLE_BASE);
    }
  }

  // ---------- 朝向 ----------
  private setFacing(facing: Facing): void {
    this.facing = facing;
    this.video?.setFacing(facing);
  }

  // ---------- 移动系统 ----------
  private tryMove(): boolean {
    if (this.moveRaf !== null || this.movePlan) return true; // 已在移动
    const dir: 1 | -1 = this.facing === 'right' ? 1 : -1;
    const W = window.innerWidth;
    const cx = this.pos ? this.pos.rx * W : W - 24 - SIZE / 2;
    const distance = randomBetween(MOVE_MIN_PX, MOVE_MAX_PX);
    const target = cx + dir * distance;
    const leftBound = MOVE_MARGIN + SIZE / 2;
    const rightBound = W - MOVE_MARGIN - SIZE / 2;
    if (target < leftBound || target > rightBound) return false;

    this.movePlan = {
      startRatio: cx / W,
      startYRatio: this.pos ? this.pos.ry : (window.innerHeight - 20 - SIZE / 2) / window.innerHeight,
      targetRatio: target / W,
      dir,
      totalRatio: Math.abs(target - cx) / W,
    };
    this.setFacing(dir === 1 ? 'right' : 'left');
    // move_runleft 是"向左奔跑"专用姿态；向右移动时用对称姿态，避免人物面朝右却用左跑动画
    const moveKey = dir === 1 ? pick(['move_crab', 'move_floatstep'] as const) : pick(MOVE_POOL);
    this.playOnce(moveKey, 'move');
    this.startMoveDrive();
    return true;
  }

  private startMoveDrive(): void {
    if (!this.movePlan || this.moveRaf !== null) return;
    // 等待视频就绪（duration 可读）后再启动位移，避免兜底时长与动画节奏脱节
    if (!this.video || this.video.duration() <= 0) {
      const token = this.moveToken;
      window.setTimeout(() => {
        if (this.moveToken === token) this.startMoveDrive();
      }, 50);
      return;
    }
    const plan = this.movePlan;
    const token = ++this.moveToken;
    this.movePlan = null;
    const step = (): void => {
      if (this.moveToken !== token || !this.video || !this.root) return;
      const duration = this.video.duration() || 10.09;
      const t = this.video.currentTime();
      const W = window.innerWidth;
      const H = window.innerHeight;
      let ratioX: number;
      if (t <= MOVE_LEAD_SEC) {
        ratioX = plan.startRatio;
      } else if (t >= duration - MOVE_TAIL_SEC) {
        ratioX = plan.targetRatio;
      } else {
        const travelWindow = Math.max(0.1, duration - MOVE_LEAD_SEC - MOVE_TAIL_SEC);
        ratioX = plan.startRatio + plan.dir * plan.totalRatio * ((t - MOVE_LEAD_SEC) / travelWindow);
      }
      this.pos = { rx: ratioX, ry: plan.startYRatio };
      const left = ratioX * W - SIZE / 2;
      const top = plan.startYRatio * H - SIZE / 2;
      this.root.style.left = left + 'px';
      this.root.style.top = top + 'px';
      if (t < duration - MOVE_TAIL_SEC) {
        this.moveRaf = requestAnimationFrame(step);
      } else {
        this.moveRaf = null;
      }
    };
    this.moveRaf = requestAnimationFrame(step);
  }

  private stopMove(): void {
    this.movePlan = null;
    this.moveToken++;
    if (this.moveRaf !== null) {
      cancelAnimationFrame(this.moveRaf);
      this.moveRaf = null;
    }
  }

  // ---------- 点击 vs 拖拽 ----------
  private onPointerDown(e: PointerEvent): void {
    if (e.button !== 0) return;
    e.preventDefault();
    (e.currentTarget as HTMLElement).setPointerCapture?.(e.pointerId);
    this.stopIdle();
    this.stopMove();
    this.drag = { active: true, dragging: false, sx: e.clientX, sy: e.clientY };
  }

  private onPointerMove(e: PointerEvent): void {
    const d = this.drag;
    if (!d.active) return;
    const dx = e.clientX - d.sx;
    const dy = e.clientY - d.sy;
    if (!d.dragging) {
      if (Math.hypot(dx, dy) < DRAG_THRESHOLD) return;
      d.dragging = true;
      this.container?.classList.add('dragging');
      this.clearLanding();
      this.playOnce(DRAG_KEY, 'user');
    }
    if (this.root) {
      this.pos = { rx: e.clientX / window.innerWidth, ry: e.clientY / window.innerHeight };
      this.root.style.left = e.clientX - SIZE / 2 + 'px';
      this.root.style.top = e.clientY - SIZE / 2 + 'px';
    }
  }

  private onPointerUp(e: PointerEvent): void {
    const d = this.drag;
    if (!d.active) return;
    const wasDragging = d.dragging;
    d.active = false;
    d.dragging = false;
    if (wasDragging) {
      this.justDragged = true;
      window.setTimeout(() => { this.justDragged = false; }, GHOST_CLICK_MS);
      this.pos = { rx: e.clientX / window.innerWidth, ry: e.clientY / window.innerHeight };
      this.container?.classList.remove('dragging');
      this.renderPos();
      this.applyLanding();
      this.resumeBase();
    } else {
      this.respondToClick();
    }
  }

  private respondToClick(): void {
    if (this.justDragged) return;
    this.stopIdle();
    this.stopMove();
    this.playOnce(pick(CLICK_POOL), 'user');
  }
}

/** 暴露 API：init(container) / setState(snapshot) / destroy */
export function init(container: HTMLElement): WhaleClient {
  const client = new WhaleClient();
  client.mount(container);
  return client;
}
