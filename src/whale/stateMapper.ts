// src/whale/stateMapper.ts —— DSH 五态 → 动画 + 气泡映射
import type { AnimKey, DshSnapshot } from './types';

/** 状态驱动的播放语义：work=循环工作池、idle=待机链、react/done=单发后回落、offline=静止 */
export type DshAnimKind = 'work' | 'idle' | 'react' | 'done' | 'offline';

export interface DshMapResult {
  kind: DshAnimKind;
  animKey: AnimKey | null;
  bubbleText: string | null;
}

function pick<T>(pool: readonly T[]): T {
  return pool[Math.floor(Math.random() * pool.length)];
}

const REACT_POOL: readonly AnimKey[] = ['react_scared', 'react_bow'];
const DONE_POOL: readonly AnimKey[] = ['click_happy', 'act_tailslap'];
const WORK_POOL: readonly AnimKey[] = ['work_cube', 'work_toycar', 'work_tap'];

/** 五态映射（与 engine.rs build_snapshot 的 state 一致：offline/attention/working/done/idle） */
export function mapDshState(snapshot: DshSnapshot): DshMapResult {
  switch (snapshot.state) {
    case 'offline':
      return { kind: 'offline', animKey: null, bubbleText: '离线了…' };
    case 'attention':
      return {
        kind: 'react',
        animKey: pick(REACT_POOL),
        bubbleText: `有 ${snapshot.attention.length} 件事需要确认`,
      };
    case 'working': {
      const task = snapshot.running[0] ?? '任务';
      return {
        kind: 'work',
        animKey: pick(WORK_POOL),
        bubbleText: `正在处理 ${task}…`,
      };
    }
    case 'done': {
      const item = snapshot.done[0];
      return {
        kind: 'done',
        animKey: pick(DONE_POOL),
        bubbleText: item ? `搞定啦~（${item}）` : '搞定啦~',
      };
    }
    case 'idle':
    default:
      return { kind: 'idle', animKey: 'idle_breath', bubbleText: null };
  }
}
