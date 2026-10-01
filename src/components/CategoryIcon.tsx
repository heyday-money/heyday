import { t as translate, useLanguage } from "../lib/i18n"
import type { ComponentProps } from 'react'
import { NativeSelect } from './ui/native-select'
import { Tag, Utensils, Coffee, ShoppingCart, Bus, Fuel, House, PlugZap, HeartPulse, ShoppingBag, Clapperboard, GraduationCap, Plane, Gift, PawPrint, Baby, Shirt, Dumbbell, Smartphone, Wifi, ShieldCheck, Wrench, Scissors, HandHeart, BriefcaseBusiness, createLucideIcon, Users, SprayCan, ReceiptText } from 'lucide-react'

const Football = createLucideIcon('Football', [
  ['circle', { cx: '12', cy: '12', r: '10', key: 'outline' }],
  ['path', { d: 'm12 7 4.8 3.5-1.8 5.6H9l-1.8-5.6L12 7Z', key: 'panel' }],
  ['path', { d: 'M12 7V2m4.8 8.5 4.7-1.6M15 16.1l2.9 4M9 16.1l-2.9 4M7.2 10.5 2.5 8.9', key: 'seams' }],
])

export const categoryIcons = [
  { key: 'tag', get label() { return translate("Other") }, icon: Tag },
  { key: 'utensils', get label() { return translate("Food") }, icon: Utensils },
  { key: 'coffee', get label() { return translate("Drinks") }, icon: Coffee },
  { key: 'shopping-cart', get label() { return translate("Groceries") }, icon: ShoppingCart },
  { key: 'bus', get label() { return translate("Transport") }, icon: Bus },
  { key: 'fuel', get label() { return translate("Fuel") }, icon: Fuel },
  { key: 'house', get label() { return translate("Housing") }, icon: House },
  { key: 'plug-zap', get label() { return translate("Utilities") }, icon: PlugZap },
  { key: 'heart-pulse', get label() { return translate("Health") }, icon: HeartPulse },
  { key: 'shopping-bag', get label() { return translate("Shopping") }, icon: ShoppingBag },
  { key: 'clapperboard', get label() { return translate("Entertainment") }, icon: Clapperboard },
  { key: 'graduation-cap', get label() { return translate("Education") }, icon: GraduationCap },
  { key: 'plane', get label() { return translate("Travel") }, icon: Plane },
  { key: 'gift', get label() { return translate("Gifts") }, icon: Gift },
  { key: 'paw-print', get label() { return translate("Pets") }, icon: PawPrint },
  { key: 'baby', get label() { return translate("Children") }, icon: Baby },
  { key: 'shirt', get label() { return translate("Clothing") }, icon: Shirt },
  { key: 'dumbbell', get label() { return translate("Fitness") }, icon: Dumbbell },
  { key: 'smartphone', get label() { return translate("Phone") }, icon: Smartphone },
  { key: 'wifi', get label() { return translate("Internet") }, icon: Wifi },
  { key: 'shield-check', get label() { return translate("Insurance") }, icon: ShieldCheck },
  { key: 'wrench', get label() { return translate("Repairs") }, icon: Wrench },
  { key: 'scissors', get label() { return translate("Personal care") }, icon: Scissors },
  { key: 'hand-heart', get label() { return translate("Donations") }, icon: HandHeart },
  { key: 'briefcase-business', get label() { return translate("Productivity & tools") }, icon: BriefcaseBusiness },
  // Keep the stored key so existing Sports categories use the updated artwork.
  { key: 'volleyball', get label() { return translate("Sports") }, icon: Football },
  { key: 'users', get label() { return translate("Family") }, icon: Users },
  { key: 'spray-can', get label() { return translate("Household supplies") }, icon: SprayCan },
  { key: 'receipt-text', get label() { return translate("Billing") }, icon: ReceiptText },
] as const

export function CategoryIcon({name}: {name?: string | null}) {
  useLanguage()

  const Icon = categoryIcons.find(icon => icon.key === name)?.icon ?? Tag
  return <Icon aria-hidden="true" className="size-4 shrink-0" />
}

export function CategorySelect({iconName, ...props}: ComponentProps<typeof NativeSelect> & {iconName?: string | null}) {
  useLanguage()

  return <span className="mt-2 flex items-center gap-2"><CategoryIcon name={iconName} /><NativeSelect {...props} /></span>
}
