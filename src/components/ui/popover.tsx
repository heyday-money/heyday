import type { ComponentProps } from 'react'
import { Popover as PopoverPrimitive } from 'radix-ui'
import { cn } from '@/lib/utils'

const Popover = PopoverPrimitive.Root
const PopoverAnchor = PopoverPrimitive.Anchor

function PopoverContent({ className, align = 'start', sideOffset = 4, container, ...props }: ComponentProps<typeof PopoverPrimitive.Content> & {
  container?: ComponentProps<typeof PopoverPrimitive.Portal>['container']
}) {
  return <PopoverPrimitive.Portal container={container}>
    <PopoverPrimitive.Content data-slot="popover-content" align={align} sideOffset={sideOffset} collisionPadding={8}
      className={cn('z-50 rounded-md border border-line bg-card text-ink shadow-md outline-none', className)} {...props} />
  </PopoverPrimitive.Portal>
}

export { Popover, PopoverAnchor, PopoverContent }
