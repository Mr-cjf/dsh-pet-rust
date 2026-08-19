// src/whale/videoManifest.ts —— 鲸鱼娘视频资源清单（T1 生成，扫描 public/thumb/ 实际文件名）
import type { AnimCategory, AnimKey } from './types';
import { convertFileSrc } from '@tauri-apps/api/core';

/** 单个动画的清单条目 */
export interface VideoManifestEntry {
  /** 相对路径或皮肤提供的路径池；播放时从路径池随机选择。 */
  file: string | readonly string[];
  /** 是否循环播放：待机/休息/移动类为 true，其余一次性动作为 false */
  loop: boolean;
  /** 动画分类 */
  category: AnimCategory;
}

/** 28 个动画的完整清单，键为 AnimKey */
export const MANIFEST: Record<AnimKey, VideoManifestEntry> = {
  idle_breath: { file: 'thumb/idle_breath.webm', loop: true, category: 'idle' },
  idle_lookaround: { file: 'thumb/idle_lookaround.webm', loop: true, category: 'idle' },
  idle_hum: { file: 'thumb/idle_hum.webm', loop: true, category: 'idle' },
  idle_stretch: { file: 'thumb/idle_stretch.webm', loop: true, category: 'idle' },
  idle_squash: { file: 'thumb/idle_squash.webm', loop: true, category: 'idle' },
  idle_yawn: { file: 'thumb/idle_yawn.webm', loop: true, category: 'idle' },
  rest_sleep: { file: 'thumb/rest_sleep.webm', loop: true, category: 'idle' },
  react_wake: { file: 'thumb/react_wake.webm', loop: false, category: 'react' },
  work_cube: { file: 'thumb/work_cube.webm', loop: true, category: 'work' },
  work_toycar: { file: 'thumb/work_toycar.webm', loop: true, category: 'work' },
  work_tap: { file: 'thumb/work_tap.webm', loop: true, category: 'work' },
  act_violin: { file: 'thumb/act_violin.webm', loop: false, category: 'act' },
  act_watergun: { file: 'thumb/act_watergun.webm', loop: false, category: 'act' },
  act_jump: { file: 'thumb/act_jump.webm', loop: false, category: 'act' },
  act_spin: { file: 'thumb/act_spin.webm', loop: false, category: 'act' },
  act_caught: { file: 'thumb/act_caught.webm', loop: false, category: 'act' },
  act_rage: { file: 'thumb/act_rage.webm', loop: true, category: 'act' },
  act_tailslap: { file: 'thumb/act_tailslap.webm', loop: false, category: 'act' },
  move_crab: { file: 'thumb/move_crab.webm', loop: true, category: 'move' },
  move_floatstep: { file: 'thumb/move_floatstep.webm', loop: true, category: 'move' },
  move_runleft: { file: 'thumb/move_runleft.webm', loop: true, category: 'move' },
  react_bow: { file: 'thumb/react_bow.webm', loop: true, category: 'react' },
  react_scared: { file: 'thumb/react_scared.webm', loop: true, category: 'react' },
  fx_bubbles: { file: 'thumb/fx_bubbles.webm', loop: false, category: 'fx' },
  click_happy: { file: 'thumb/click_happy.webm', loop: false, category: 'click' },
  click_shy: { file: 'thumb/click_shy.webm', loop: false, category: 'click' },
  click_tsundere: { file: 'thumb/click_tsundere.webm', loop: false, category: 'click' },
  drag_hang: { file: 'thumb/drag_hang.webm', loop: false, category: 'drag' },
};

export default MANIFEST;

/** 保存内置皮肤的原始 file，用于切回内置时恢复。 */
const BUILTIN_FILES: Record<AnimKey, string> = Object.fromEntries(
  Object.entries(MANIFEST).map(([k, v]) => [k, typeof v.file === 'string' ? v.file : v.file[0]]),
) as Record<AnimKey, string>;

/** 事件名写法对应的预置动画键。显式动画键配置优先于事件名配置。 */
const EVENT_KEYS: Record<string, readonly AnimKey[]> = {
  idle: ['idle_breath'],
  work: ['work_cube', 'work_toycar', 'work_tap'],
  react: ['react_scared', 'react_bow'],
  waiting: ['react_scared', 'react_bow'],
  done: ['click_happy', 'act_tailslap'],
  complete: ['click_happy', 'act_tailslap'],
  error: ['act_rage'],
  click: ['click_happy', 'click_shy', 'click_tsundere'],
  drag: ['drag_hang'],
  move: ['move_crab', 'move_floatstep', 'move_runleft'],
  idle_random: ['idle_lookaround', 'idle_hum', 'idle_stretch', 'idle_yawn', 'idle_squash'],
  random: ['act_violin', 'act_watergun', 'act_jump', 'act_caught', 'act_rage', 'act_tailslap', 'fx_bubbles', 'react_bow'],
  easter_egg: ['act_spin', 'fx_bubbles', 'act_caught'],
  rest: ['rest_sleep', 'react_wake'],
};

let SKIN_EVENT_FILES: Record<string, readonly string[]> = {};

/** 恢复内置皮肤动画路径。 */
export function resetSkin(): void {
  SKIN_EVENT_FILES = {};
  for (const key of Object.keys(MANIFEST) as AnimKey[]) {
    MANIFEST[key].file = BUILTIN_FILES[key];
  }
}

/** 返回动画键对应的有效路径池；事件名配置作为未显式配置键的回退。 */
export function animationFiles(key: AnimKey): readonly string[] {
  const value = MANIFEST[key].file;
  if (Array.isArray(value)) return value;
  return typeof value === 'string' && value ? [value] : [];
}

/** 切换皮肤：把后端绝对路径数组映射应用为清单路径池。 */
export function applySkin(animations: Record<string, string[]> | null): void {
  // 每次切换都先恢复内置路径，避免上一套皮肤的键残留。
  resetSkin();
  if (!animations) return;
  for (const [key, paths] of Object.entries(animations)) {
    const files = paths
      .filter((path) => typeof path === 'string' && path)
      .map((path) => convertFileSrc(path));
    if (files.length === 0) continue;
    if (key in MANIFEST) {
      MANIFEST[key as AnimKey].file = files;
    } else {
      SKIN_EVENT_FILES[key] = files;
    }
  }
  for (const [event, keys] of Object.entries(EVENT_KEYS)) {
    const files = SKIN_EVENT_FILES[event];
    if (!files) continue;
    for (const key of keys) {
      const explicit = animations[key];
      if (!explicit || explicit.filter(Boolean).length === 0) {
        MANIFEST[key].file = files;
      }
    }
  }
}

/** 每次调用返回一个随机路径，数组配置不会固定使用第一个文件。 */
export function pickAnimationFile(key: AnimKey): string {
  const files = animationFiles(key);
  return files[Math.floor(Math.random() * files.length)] ?? BUILTIN_FILES[key];
}
