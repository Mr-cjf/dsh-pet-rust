// src/renderer.ts —— 前端渲染编排：全屏容器 + whaleClient + dsh-state 订阅
import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';
import type { AnimKey, DshSnapshot } from './whale/types';
import { init as initWhale, WhaleClient } from './whale/whaleClient';
import { applySkin, resetSkin } from './whale/videoManifest';

interface DshStateEvent {
  state: string;
  attention: string[];
  running: string[];
  done: string[];
}

const STATE_KEYS: readonly string[] = ['offline', 'attention', 'working', 'done', 'idle', 'default'];
const ACTION_KEYS: Readonly<Record<string, AnimKey>> = {
  '7': 'move_crab',
  '9': 'click_happy',
  '0': 'click_shy',
  '-': 'click_tsundere',
};

function hasTauriRuntime(): boolean {
  return typeof window !== 'undefined' && ('__TAURI_INTERNALS__' in window || '__TAURI__' in window);
}

function toSnapshot(state: string): DshSnapshot {
  return { state, attention: [], running: [], done: [] };
}

async function applyConfiguredTheme(client: WhaleClient): Promise<void> {
  try {
    const theme = await invoke<{ theme_id: string; animations: Record<string, string[]> | null }>('get_theme');
    if (theme.animations) {
      applySkin(theme.animations);
    } else {
      resetSkin();
    }
    client.reloadCurrent();
  } catch {
    resetSkin();
  }
}

export function initRenderer(): void {
  const container = document.getElementById('root') as HTMLElement;
  container.classList.add('whale-container');

  const client: WhaleClient = initWhale(container);

  // 键盘调试：1-6 模拟五态，7/9/0/- 触发对应视频动画，8 转向
  const onKeydown = (e: KeyboardEvent): void => {
    const n = Number(e.key);
    if (n >= 1 && n <= STATE_KEYS.length) {
      const state = STATE_KEYS[n - 1] ?? 'idle';
      client.setState(toSnapshot(state));
      return;
    }
    if (e.key === '8') {
      client.turnAction();
      return;
    }
    const action = ACTION_KEYS[e.key];
    if (action) client.actAction(action);
  };
  window.addEventListener('keydown', onKeydown);

  // 订阅 Rust 引擎 dsh-state 事件
  let unlisten: (() => void) | null = null;
  const release = (): void => { unlisten?.(); };
  window.addEventListener('beforeunload', release, { once: true });
  if (hasTauriRuntime()) {
    void (async () => {
      try {
        unlisten = await listen<DshStateEvent>('dsh-state', (event) => {
          const snap = event.payload;
          if (!snap) return;
          client.setState({
            state: snap.state,
            attention: snap.attention ?? [],
            running: snap.running ?? [],
            done: snap.done ?? [],
          });
        });

        // 免打扰切换
        await listen<boolean>('pet-dnd', (event) => {
          client.setDnd(!!event.payload);
        });

        // 皮肤切换事件（托盘点击）
        await listen<string>('theme-changed', () => {
          void applyConfiguredTheme(client);
        });

        // 启动时加载已配置的皮肤
        await applyConfiguredTheme(client);

      } catch (err) {
        console.warn('[dsh-pet] 事件监听失败', err);
      }
    })();
  } else {
    console.warn('[dsh-pet] 未检测到 Tauri 运行环境，跳过 dsh-state 监听（纯 vite dev）');
  }

  // 默认 idle
  client.setState(toSnapshot('idle'));
}
