import { invoke } from '@tauri-apps/api/core'
export type LogoChange = { kind: 'custom'; data: string } | { kind: 'default' } | { kind: 'none' }
const cache = new Map<string, Promise<string>>()
export function getLogoAsset(id: string) {
  let pending = cache.get(id)
  if (!pending) {
    pending = invoke<string>('get_logo_asset', { id }).catch(error => { cache.delete(id); throw error })
    cache.set(id, pending)
    if (cache.size > 128) cache.delete(cache.keys().next().value!)
  }
  return pending
}

async function prepareSvg(file: File): Promise<string> {
  const document = new DOMParser().parseFromString(await file.text(), 'image/svg+xml')
  const root = document.documentElement
  if (document.querySelector('parsererror') || root.localName !== 'svg' || root.namespaceURI !== 'http://www.w3.org/2000/svg' || document.doctype) {
    throw new Error('This SVG could not be read. Choose another image.')
  }
  const viewBox = root.getAttribute('viewBox')?.trim().split(/[\s,]+/).map(Number)
  const dimension = (name: string) => {
    const value = root.getAttribute(name) ?? ''
    return /^\d+(?:\.\d+)?(?:px)?$/.test(value) ? parseFloat(value) : NaN
  }
  let width = dimension('width'), height = dimension('height')
  if (!(width > 0 && height > 0)) {
    width = viewBox?.length === 4 ? viewBox[2] : 128
    height = viewBox?.length === 4 ? viewBox[3] : 128
  }
  if (!Number.isFinite(width) || !Number.isFinite(height) || width <= 0 || height <= 0) throw new Error('This SVG has invalid dimensions.')
  // Give viewBox-only SVGs explicit, bounded raster dimensions for WebView support.
  if (!root.hasAttribute('viewBox')) root.setAttribute('viewBox', `0 0 ${width} ${height}`)
  const scale = 128 / Math.max(width, height)
  root.setAttribute('width', String(Math.max(1, Math.round(width * scale))))
  root.setAttribute('height', String(Math.max(1, Math.round(height * scale))))
  const url = `data:image/svg+xml;charset=utf-8,${encodeURIComponent(new XMLSerializer().serializeToString(root))}`
  try {
    // SVG is decoded only as an image, never inserted into the page. Image mode
    // disables scripts and external resources; only the resulting PNG is stored.
    const img = new Image()
    img.src = url
    await img.decode()
    return rasterize(img, img.naturalWidth, img.naturalHeight)
  } catch { throw new Error('This SVG could not be read. Choose another image.') }
}

function rasterize(image: CanvasImageSource, width: number, height: number): string {
  const scale = Math.min(1, 128 / Math.max(width, height))
  const canvas = document.createElement('canvas')
  canvas.width = Math.max(1, Math.round(width * scale))
  canvas.height = Math.max(1, Math.round(height * scale))
  const context = canvas.getContext('2d')
  if (!context) throw new Error('Could not prepare the image. Please try again.')
  context.imageSmoothingQuality = 'high'
  context.drawImage(image, 0, 0, canvas.width, canvas.height)
  return canvas.toDataURL('image/png')
}

export async function prepareLogo(file: File): Promise<string> {
  if (file.size > 5 * 1024 * 1024) throw new Error('Choose an image smaller than 5 MB.')
  const header = new Uint8Array(await file.slice(0, 12).arrayBuffer())
  const png = header[0] === 137 && header[1] === 80 && header[2] === 78 && header[3] === 71
  const jpeg = header[0] === 255 && header[1] === 216 && header[2] === 255
  const webp = String.fromCharCode(...header.slice(0, 4)) === 'RIFF' && String.fromCharCode(...header.slice(8, 12)) === 'WEBP'
  if (!png && !jpeg && !webp) {
    if (file.type === 'image/svg+xml' || /\.svg$/i.test(file.name)) return prepareSvg(file)
    throw new Error('Choose a PNG, JPG, WebP, or SVG image.')
  }
  let bitmap: ImageBitmap
  try { bitmap = await createImageBitmap(file) } catch { throw new Error('This image could not be read. Choose another image.') }
  try {
    if (!bitmap.width || !bitmap.height || bitmap.width * bitmap.height > 16_000_000) throw new Error('Choose an image with no more than 16 million pixels.')
    return rasterize(bitmap, bitmap.width, bitmap.height)
  } finally { bitmap.close() }
}
