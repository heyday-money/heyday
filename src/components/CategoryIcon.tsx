import type { ComponentProps } from 'react'
import { NativeSelect } from './ui/native-select'
import { Tag, Utensils, Coffee, ShoppingCart, Bus, Fuel, House, PlugZap, HeartPulse, ShoppingBag, Clapperboard, GraduationCap, Plane, Gift, PawPrint, Baby, Shirt, Dumbbell, Smartphone, Wifi, ShieldCheck, Wrench, Scissors, HandHeart, BriefcaseBusiness, createLucideIcon, Users, SprayCan, ReceiptText } from 'lucide-react'

const Football = createLucideIcon('Football', [
  ['circle', { cx: '12', cy: '12', r: '10', key: 'outline' }],
  ['path', { d: 'm12 7 4.8 3.5-1.8 5.6H9l-1.8-5.6L12 7Z', key: 'panel' }],
  ['path', { d: 'M12 7V2m4.8 8.5 4.7-1.6M15 16.1l2.9 4M9 16.1l-2.9 4M7.2 10.5 2.5 8.9', key: 'seams' }],
])

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
  { key: 'briefcase-business', label: 'Productivity & tools', icon: BriefcaseBusiness },
  // Keep the stored key so existing Sports categories use the updated artwork.
  { key: 'volleyball', label: 'Sports', icon: Football },
  { key: 'users', label: 'Family', icon: Users },
  { key: 'spray-can', label: 'Household supplies', icon: SprayCan },
  { key: 'receipt-text', label: 'Billing', icon: ReceiptText },
] as const

export function CategoryIcon({name}: {name?: string | null}) {
  const Icon = categoryIcons.find(icon => icon.key === name)?.icon ?? Tag
  return <Icon aria-hidden="true" className="size-4 shrink-0" />
}

export function CategorySelect({iconName, ...props}: ComponentProps<typeof NativeSelect> & {iconName?: string | null}) {
  return <span className="mt-2 flex items-center gap-2"><CategoryIcon name={iconName} /><NativeSelect {...props} /></span>
}
