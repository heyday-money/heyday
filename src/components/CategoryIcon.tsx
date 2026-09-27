import type { ComponentProps } from 'react'
import { NativeSelect } from './ui/native-select'
import { Tag, Utensils, Coffee, ShoppingCart, Bus, Fuel, House, PlugZap, HeartPulse, ShoppingBag, Clapperboard, GraduationCap, Plane, Gift, PawPrint, Baby, Shirt, Dumbbell, Smartphone, Wifi, ShieldCheck, Wrench, Scissors, HandHeart } from 'lucide-react'

export const categoryIcons = [
  { key: 'tag', label: 'Other', icon: Tag },
  { key: 'utensils', label: 'Food', icon: Utensils },
  { key: 'coffee', label: 'Drinks', icon: Coffee },
  { key: 'shopping-cart', label: 'Groceries', icon: ShoppingCart },
  { key: 'bus', label: 'Transport', icon: Bus },
  { key: 'fuel', label: 'Fuel', icon: Fuel },
  { key: 'house', label: 'Housing', icon: House },
  { key: 'plug-zap', label: 'Utilities', icon: PlugZap },
  { key: 'heart-pulse', label: 'Health', icon: HeartPulse },
  { key: 'shopping-bag', label: 'Shopping', icon: ShoppingBag },
  { key: 'clapperboard', label: 'Entertainment', icon: Clapperboard },
  { key: 'graduation-cap', label: 'Education', icon: GraduationCap },
  { key: 'plane', label: 'Travel', icon: Plane },
  { key: 'gift', label: 'Gifts', icon: Gift },
  { key: 'paw-print', label: 'Pets', icon: PawPrint },
  { key: 'baby', label: 'Children', icon: Baby },
  { key: 'shirt', label: 'Clothing', icon: Shirt },
  { key: 'dumbbell', label: 'Fitness', icon: Dumbbell },
  { key: 'smartphone', label: 'Phone', icon: Smartphone },
  { key: 'wifi', label: 'Internet', icon: Wifi },
  { key: 'shield-check', label: 'Insurance', icon: ShieldCheck },
  { key: 'wrench', label: 'Repairs', icon: Wrench },
  { key: 'scissors', label: 'Personal care', icon: Scissors },
  { key: 'hand-heart', label: 'Donations', icon: HandHeart },
] as const

export function CategoryIcon({name}: {name?: string | null}) {
  const Icon = categoryIcons.find(icon => icon.key === name)?.icon ?? Tag
  return <Icon aria-hidden="true" className="size-4 shrink-0" />
}

export function CategorySelect({iconName, ...props}: ComponentProps<typeof NativeSelect> & {iconName?: string | null}) {
  return <span className="mt-2 flex items-center gap-2"><CategoryIcon name={iconName} /><NativeSelect {...props} /></span>
}
