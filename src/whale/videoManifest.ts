// src/whale/videoManifest.ts —— 鲸鱼娘视频资源清单（T1 生成，扫描 public/thumb/ 实际文件名）
import type { AnimCategory, AnimKey } from './types';

/** 单个动画的清单条目 */
export interface VideoManifestEntry {
  /** 相对路径：'/thumb/' + 文件名（Tauri 资产协议相对路径） */
  file: string;
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
