import { convertFileSrc, invoke } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import './theme-editor.css';

type ThemeInfo = { id: string; name: string };
type Definition = { id: string; name: string; animations: Record<string, string[]> };
type Group = { key: string; label: string; required: boolean };

const GROUPS: Group[] = [
  { key: 'idle_breath', label: '空闲呼吸', required: true },
  { key: 'work', label: '工作', required: true },
  { key: 'react', label: '等待审批', required: true },
  { key: 'done', label: '完成', required: true },
  { key: 'error', label: '出错', required: true },
  { key: 'click', label: '点击', required: true },
  { key: 'drag', label: '拖拽', required: true },
  { key: 'move', label: '移动', required: true },
  { key: 'idle_random', label: '待机随机动作', required: false },
  { key: 'random', label: '随机动作池', required: false },
  { key: 'easter_egg', label: '彩蛋', required: false },
  { key: 'rest', label: '休息 / 起床', required: false },
];

const root = document.querySelector<HTMLDivElement>('#root')!;
const state: { id?: string; name: string; animations: Record<string, string[]> } = { name: '我的皮肤', animations: {} };
const preview = document.createElement('video');
preview.controls = true;
preview.muted = true;
preview.className = 'theme-preview-video';
const localFiles = new Map<string, File>();

function el<K extends keyof HTMLElementTagNameMap>(tag: K, cls?: string): HTMLElementTagNameMap[K] {
  const node = document.createElement(tag);
  if (cls) node.className = cls;
  return node;
}

function render(): void {
  root.replaceChildren();
  const shell = el('main', 'editor-shell');
  const header = el('header', 'editor-header');
  header.innerHTML = '<div><div class="eyebrow">DSH PET</div><h1>皮肤编辑器</h1><p>用可视化方式组合动画，不需要编辑配置文件。</p></div>';
  const close = el('button', 'ghost-button'); close.textContent = '关闭'; close.onclick = () => void getCurrentWindow().close(); header.append(close);
  shell.append(header);

  const toolbar = el('section', 'editor-toolbar');
  const name = el('input') as HTMLInputElement; name.type = 'text'; name.value = state.name; name.placeholder = '皮肤名称'; name.oninput = () => { state.name = name.value; };
  const select = el('select') as HTMLSelectElement;
  const load = el('button', 'secondary-button'); load.textContent = '载入已有皮肤'; load.onclick = () => void loadTheme(select.value);
  toolbar.append(labelled('名称', name), labelled('已有皮肤', select), load);
  shell.append(toolbar);

  const content = el('div', 'editor-content');
  const list = el('section', 'event-list');
  for (const group of GROUPS) list.append(eventCard(group));
  content.append(list);
  const side = el('aside', 'preview-panel');
  side.innerHTML = '<div class="panel-kicker">预览</div><h2>动画预览</h2><p>选择动画文件后，可在这里检查画面。</p>';
  side.append(preview);
  const hint = el('div', 'preview-hint'); hint.textContent = '支持 webm；一个事件可以添加多个文件，播放时随机选择。'; side.append(hint);
  content.append(side); shell.append(content);

  const footer = el('footer', 'editor-footer');
  const status = el('div', 'editor-status'); status.id = 'editor-status'; footer.append(status);
  const save = el('button', 'primary-button'); save.textContent = '保存并应用'; save.onclick = () => void saveTheme(); footer.append(save); shell.append(footer);
  root.append(shell);
  void populateThemes(select);
}

function labelled(text: string, input: HTMLElement): HTMLElement {
  const wrap = el('label', 'field'); const caption = el('span'); caption.textContent = text; wrap.append(caption, input); return wrap;
}

function eventCard(group: Group): HTMLElement {
  const card = el('article', 'event-card');
  const top = el('div', 'event-top'); const title = el('h3'); title.textContent = group.label; top.append(title);
  const badge = el('span', group.required ? 'required' : 'optional'); badge.textContent = group.required ? '必需' : '可选'; top.append(badge); card.append(top);
  const slots = el('div', 'file-slots');
  const files = state.animations[group.key] ?? [];
  for (const file of files) slots.append(fileRow(group.key, file, slots));
  const add = el('button', 'add-file'); add.textContent = '+ 添加动画文件'; add.onclick = () => void chooseFile(group.key, slots); card.append(slots, add); return card;
}

function fileRow(key: string, file: string, slots: HTMLElement): HTMLElement {
  const row = el('div', 'file-row'); const name = el('span'); name.textContent = file.split(/[\\/]/).pop() || file; name.title = file;
  const play = el('button', 'icon-button'); play.textContent = '预览'; play.onclick = () => playFile(file);
  const remove = el('button', 'icon-button danger'); remove.textContent = '移除'; remove.onclick = () => { state.animations[key] = (state.animations[key] ?? []).filter((v) => v !== file); row.remove(); };
  row.append(name, play, remove); slots.append(row); return row;
}

async function chooseFile(key: string, slots: HTMLElement): Promise<void> {
  try {
    const paths = await invoke<string[]>('pick_webm_files');
    for (const path of paths) {
      if (!state.animations[key]?.includes(path)) {
        state.animations[key] = [...(state.animations[key] ?? []), path];
        fileRow(key, path, slots);
      }
    }
    if (paths.length) setStatus(`已添加 ${paths.length} 个动画文件，可继续添加。`);
  } catch (error) {
    setStatus(String(error), true);
  }
}

function playFile(file: string): void {
  const local = localFiles.get(file);
  if (local) preview.src = URL.createObjectURL(local);
  else if (/^https?:|^blob:|^asset:/.test(file)) preview.src = file;
  else {
    try { preview.src = convertFileSrc(file); }
    catch { setStatus('无法预览该动画文件。', true); return; }
  }
  void preview.play().catch(() => undefined);
}

async function populateThemes(select: HTMLSelectElement): Promise<void> {
  try { const themes = await invoke<ThemeInfo[]>('get_themes'); select.replaceChildren(new Option('新建皮肤', '')); for (const theme of themes) select.add(new Option(theme.name, theme.id)); }
  catch { setStatus('无法读取皮肤列表', true); }
}

async function loadTheme(id: string): Promise<void> {
  if (!id) { state.id = undefined; state.name = '我的皮肤'; state.animations = {}; render(); return; }
  try { const data = await invoke<Definition>('get_theme_definition', { id }); state.id = data.id; state.name = data.name; state.animations = data.animations ?? {}; render(); }
  catch { setStatus('载入失败', true); }
}

async function saveTheme(): Promise<void> {
  const missing = GROUPS.filter((group) => group.required && !(state.animations[group.key]?.length));
  if (missing.length) { setStatus(`请至少为这些事件添加动画：${missing.map((v) => v.label).join('、')}`, true); return; }
  try { const info = await invoke<ThemeInfo>('save_theme', { id: state.id ?? null, name: state.name, animations: state.animations }); state.id = info.id; setStatus('已保存，皮肤已立即应用。'); await new Promise((r) => setTimeout(r, 500)); render(); }
  catch (error) { setStatus(String(error), true); }
}

function setStatus(text: string, error = false): void { const node = document.querySelector('#editor-status'); if (node) { node.textContent = text; node.className = `editor-status${error ? ' error' : ''}`; } }

export function initThemeEditor(): void { document.body.classList.add('editor-page'); render(); }
