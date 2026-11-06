import { useState } from 'react'
import { Check, CreditCard, ExternalLink, TriangleAlert } from 'lucide-react'
import { cloudManageSubscription } from '../lib/desktop'
import { getLocale, t, useLanguage } from '../lib/i18n'
import { Button } from './ui/button'
import { useCloudSession } from './CloudSessionProvider'

export function CloudSubscription({ disabled }: { disabled: boolean }) {
  useLanguage()
  const { status, syncAccess } = useCloudSession()
  const [opening, setOpening] = useState(false)
  const [error, setError] = useState('')
  const subscription = status.subscription
  const entitlement = subscription?.entitlement
  const subscriptions = subscription?.billing?.subscriptions ?? []
  const allowed = syncAccess === 'allowed'
  const unknown = syncAccess === 'unknown'
  const unavailable = entitlement?.reason === 'not_configured'
  const ongoing = subscriptions.filter(s => ['active', 'trialing'].includes(s.status))
  const ending = allowed && entitlement?.reason !== 'beta_access' && ongoing.length > 0 && ongoing.every(s => s.cancelAt)
  const paymentIssue = subscriptions.some(s => ['past_due', 'unpaid', 'incomplete', 'paused'].includes(s.status))
  const title = unknown ? t('Subscription status unavailable')
    : unavailable ? t('Cloud sync unavailable')
    : allowed ? entitlement?.reason === 'beta_access' ? t('Beta access active') : ending ? t('Cancellation scheduled') : entitlement?.reason === 'trial' ? t('Trial active') : t('Subscription active')
    : paymentIssue ? t('Payment needs attention') : subscriptions.length ? t('Cloud sync access ended') : t('Choose a subscription')
  const date = entitlement?.accessUntil && Number.isFinite(Date.parse(entitlement.accessUntil))
    ? new Date(entitlement.accessUntil).toLocaleString(getLocale(), { dateStyle: 'medium', timeStyle: 'short' }) : null
  const Icon = allowed ? Check : unknown || unavailable || paymentIssue ? TriangleAlert : CreditCard
  async function manage() {
    setOpening(true); setError('')
    try { await cloudManageSubscription() }
    catch { setError('Could not open subscription management. Please try again.') }
    finally { setOpening(false) }
  }
  return <section aria-labelledby="cloud-subscription-heading" className="rounded-2xl border border-line bg-card p-5">
    <div className="flex items-start gap-3">
      <span className={`flex size-9 shrink-0 items-center justify-center rounded-full ${allowed ? 'bg-emerald-600 text-white' : 'bg-soft text-muted'}`}><Icon className="size-4" aria-hidden="true" /></span>
      <div className="min-w-0 space-y-2">
        <h4 id="cloud-subscription-heading" className="font-semibold">{t('Account subscription')}</h4>
        <p role="status" className="text-sm font-medium">{title}</p>
        {unknown ? <p className="text-sm text-muted">{t('Connect and refresh status to verify your cloud sync access. Your local finances remain available.')}</p>
          : unavailable ? <p className="text-sm text-muted">{t('Cloud sync is temporarily unavailable on the service. Your local finances and cloud data are retained.')}</p>
          : allowed ? <p className="text-sm text-muted">{ending ? t('Cloud sync continues until {date}. Your encrypted cloud data will be retained after access ends.', { date })
            : entitlement?.reason === 'trial' ? t('Trial access until {date}. Manage renewal or cancellation on the website.', { date })
            : t('Cloud sync access until {date}.', { date })}</p>
          : <p className="text-sm text-muted">{paymentIssue ? t('Review your payment or subscription on the website to restore sync. Your local finances and encrypted cloud data are retained.')
            : t('An active subscription or trial is needed to sync. Your local finances and encrypted cloud data are retained.')}</p>}
      </div>
    </div>
    <div className="mt-4 space-y-3">
      <Button variant="outline" disabled={disabled || opening} onClick={() => void manage()}><ExternalLink aria-hidden="true" />{t('Manage subscription on website')}</Button>
      <p className="text-xs text-muted">{t('Use {email} on the website. Subscription status refreshes when you return to the app.', { email: status.account?.email ?? '' })}</p>
      {error && <p role="alert" className="text-sm text-red-600 dark:text-red-400">{t(error)}</p>}
    </div>
  </section>
}
