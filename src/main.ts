// dsh-pet 前端入口
import './styles.css'
import { initRenderer } from './renderer'
import { initThemeEditor } from './theme-editor'

if (new URLSearchParams(window.location.search).has('editor')) initThemeEditor()
else initRenderer()
