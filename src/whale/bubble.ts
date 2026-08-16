// src/whale/bubble.ts —— 气泡 DOM（跟随鲸鱼娘，定位在其上方）
import { append, el, style } from './dom';

export class Bubble {
  private root: HTMLElement | null = null;
  private timer: number | null = null;

  mount(parent: HTMLElement): void {
    this.root = el('div', 'whale-bubble');
    style(this.root, { display: 'none', opacity: '0' });
    append(parent, this.root);
  }

  show(text: string): void {
    if (!this.root) return;
    if (this.timer !== null) {
      clearTimeout(this.timer);
      this.timer = null;
    }
    this.root.textContent = text;
    this.root.style.display = 'block';
    // 强制回流后淡入
    void this.root.offsetWidth;
    this.root.style.opacity = '1';
  }

  hide(): void {
    if (!this.root) return;
    this.root.style.opacity = '0';
    if (this.timer !== null) clearTimeout(this.timer);
    this.timer = window.setTimeout(() => {
      if (this.root) this.root.style.display = 'none';
      this.timer = null;
    }, 180);
  }

  setEnabled(enabled: boolean): void {
    if (enabled) return;
    this.hide();
  }

  unmount(): void {
    if (this.timer !== null) clearTimeout(this.timer);
    this.timer = null;
    if (this.root?.parentNode) this.root.parentNode.removeChild(this.root);
    this.root = null;
  }
}
