import { useRef } from 'react'

export function SidebarResizeHandle({ width, maxWidth, onResize }: {
  width: number
  maxWidth: number
  onResize: (width: number) => void
}) {
  const drag = useRef<{ x: number; width: number } | null>(null)
  const resize = (value: number) => onResize(Math.round(Math.min(maxWidth, Math.max(180, value))))
  return <div role="separator" aria-label="Resize sidebar" aria-orientation="vertical"
    aria-valuemin={180} aria-valuemax={maxWidth} aria-valuenow={width} tabIndex={0}
    title="Drag to resize. Use arrow keys to adjust."
    className="absolute inset-y-0 -right-1 z-20 w-2 touch-none cursor-col-resize select-none hover:bg-brand/20 focus-visible:bg-brand/20 focus-visible:outline-2 focus-visible:outline-brand"
    onPointerDown={event => {
      if (event.button !== 0) return
      event.preventDefault()
      event.currentTarget.focus()
      drag.current = { x: event.clientX, width }
      event.currentTarget.setPointerCapture(event.pointerId)
    }}
    onPointerMove={event => {
      if (drag.current) resize(drag.current.width + event.clientX - drag.current.x)
    }}
    onPointerUp={event => {
      drag.current = null
      if (event.currentTarget.hasPointerCapture(event.pointerId)) event.currentTarget.releasePointerCapture(event.pointerId)
    }}
    onPointerCancel={() => { drag.current = null }}
    onLostPointerCapture={() => { drag.current = null }}
    onKeyDown={event => {
      const values: Record<string, number> = { ArrowLeft: width - 10, ArrowRight: width + 10, Home: 180, End: maxWidth }
      if (event.key in values) { event.preventDefault(); resize(values[event.key]) }
    }}
  />
}
