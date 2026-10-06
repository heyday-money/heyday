import { providerIcon } from '../lib/subscription-providers'
import type { Subscription } from '../lib/desktop'
import { LogoImage, useLogoAsset } from './LogoImage'

export function SubscriptionLogo({ subscription }: { subscription: Subscription }) {
  const src = useLogoAsset(subscription.logo_asset_id)
  return <LogoImage src={src ?? providerIcon(subscription.provider_icon)} name={subscription.name} className="size-9 rounded-lg text-base" />
}
