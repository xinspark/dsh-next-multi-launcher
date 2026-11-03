import { mount } from 'svelte';
import App from './App.svelte';
import './styles/tokens.css';
// 样式按层拆分，导入顺序 = CSS 层叠顺序（不要调换）
import './styles/base.css';
import './styles/chrome.css';
import './styles/panes.css';
import './styles/views.css';

// 输入模态：鼠标操作不画焦点环、键盘导航才画。
// 判据来自官方 design/parts/focus.css 的 `html[data-input-modality=pointer]`，这里负责把模态写进 DOM。
const root = document.documentElement;
const setModality = (mode: 'pointer' | 'keyboard') => root.setAttribute('data-input-modality', mode);
setModality('pointer');
window.addEventListener('pointerdown', () => setModality('pointer'), true);
window.addEventListener('keydown', () => setModality('keyboard'), true);

const app = mount(App, { target: document.getElementById('app')! });

export default app;
