// src/whale/dom.ts —— 原生 DOM 创建辅助（替代 JSX）
export function el<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  className?: string,
  text?: string,
): HTMLElementTagNameMap[K] {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (text !== undefined) node.textContent = text;
  return node;
}

export function style(node: HTMLElement, props: Partial<CSSStyleDeclaration>): HTMLElement {
  Object.assign(node.style, props);
  return node;
}

export function append(parent: HTMLElement, ...children: (HTMLElement | null | undefined)[]): HTMLElement {
  for (const child of children) {
    if (child) parent.appendChild(child);
  }
  return parent;
}

export function clear(node: HTMLElement): void {
  while (node.firstChild) node.removeChild(node.firstChild);
}
