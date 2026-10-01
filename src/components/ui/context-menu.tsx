import * as React from 'react'
import { ContextMenu as ContextMenuPrimitive } from 'radix-ui'
import { cn } from '@/lib/utils'

function ContextMenu(props: React.ComponentProps<typeof ContextMenuPrimitive.Root>) {
  return <ContextMenuPrimitive.Root {...props} />
}
function ContextMenuTrigger(props: React.ComponentProps<typeof ContextMenuPrimitive.Trigger>) {
  return <ContextMenuPrimitive.Trigger data-slot="context-menu-trigger" {...props} />
}
function ContextMenuContent({ className, ...props }: React.ComponentProps<typeof ContextMenuPrimitive.Content>) {
  return <ContextMenuPrimitive.Portal><ContextMenuPrimitive.Content data-slot="context-menu-content" className={cn('z-50 min-w-32 overflow-hidden rounded-lg border border-line bg-card p-1 text-ink shadow-md', className)} {...props} /></ContextMenuPrimitive.Portal>
}
function ContextMenuItem({ className, ...props }: React.ComponentProps<typeof ContextMenuPrimitive.Item>) {
  return <ContextMenuPrimitive.Item data-slot="context-menu-item" className={cn('relative flex cursor-default select-none items-center gap-2 rounded-md px-2 py-1.5 text-sm outline-none focus:bg-soft data-[disabled]:pointer-events-none data-[disabled]:opacity-50 [&_svg]:size-4 [&_svg]:shrink-0', className)} {...props} />
}
export { ContextMenu, ContextMenuTrigger, ContextMenuContent, ContextMenuItem }
