import { readFile, writeFile } from 'node:fs/promises'
import { compile } from 'tailwindcss'

const root = new URL('../', import.meta.url)
const source = await readFile(new URL('src-tauri/assets/auth-callback.template.html', root), 'utf8')
const [theme, preflight] = await Promise.all([
  readFile(new URL(import.meta.resolve('tailwindcss/theme.css')), 'utf8'),
  readFile(new URL(import.meta.resolve('tailwindcss/preflight.css')), 'utf8'),
])
const candidates = [...source.matchAll(/class="([^"]+)"/g)].flatMap(match => match[1].split(/\s+/))
const compiler = await compile(`${theme}\n${preflight}\n@tailwind utilities;`)
const html = source.replace('__STYLES__', compiler.build(candidates))
await writeFile(new URL('src-tauri/assets/auth-callback.html', root), html)
