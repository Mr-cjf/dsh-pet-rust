// src/whale/types.ts —— 鲸鱼娘动画类型定义（T1 生成）

/** 28 个鲸鱼娘动画的英文键（联合类型） */
export type AnimKey =
  | 'idle_breath'
  | 'idle_lookaround'
  | 'idle_hum'
  | 'idle_stretch'
  | 'idle_squash'
  | 'idle_yawn'
  | 'rest_sleep'
  | 'react_wake'
  | 'work_cube'
  | 'work_toycar'
  | 'work_tap'
  | 'act_violin'
  | 'act_watergun'
  | 'act_jump'
  | 'act_spin'
  | 'act_caught'
  | 'act_rage'
  | 'act_tailslap'
  | 'move_crab'
  | 'move_floatstep'
  | 'move_runleft'
  | 'react_bow'
  | 'react_scared'
  | 'fx_bubbles'
  | 'click_happy'
  | 'click_shy'
  | 'click_tsundere'
  | 'drag_hang';

/** 动画分类 */
export type AnimCategory =
  | 'idle'
  | 'move'
  | 'click'
  | 'drag'
  | 'react'
  | 'work'
  | 'act'
  | 'fx';

/** DSH 快照 */
export interface DshSnapshot {
  state: string;
  attention: string[];
  running: string[];
  done: string[];
}

/** 命中矩形 */
export interface HitRect {
  x: number;
  y: number;
  w: number;
  h: number;
}
