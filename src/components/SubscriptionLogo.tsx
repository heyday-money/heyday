import type { Subscription } from '../lib/desktop'
import { LogoImage, useLogoAsset } from './LogoImage'

export function SubscriptionLogo({ subscription }: { subscription: Subscription }) {
  const src = useLogoAsset(subscription.logo_asset_id)
  return <LogoImage src={src} name={subscription.name} className="size-9 rounded-lg text-base" />
}
