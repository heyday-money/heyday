import { Link } from '@tanstack/react-router'
import { useCloudSession } from './CloudSessionProvider'
import { t, useLanguage } from '../lib/i18n'

export function SidebarUser({ collapsed }: { collapsed: boolean }) {
  useLanguage()
  const { status: { account } } = useCloudSession()
  if (!account) return null
  const parts = account.email.split('@')[0].split(/[._+\-\s]+/u).filter(Boolean)
  const initials = Array.from(parts.slice(0, 2).map(part => Array.from(part)[0]).join('').toLocaleUpperCase()).slice(0, 2).join('') || '?'
  const label = t('Account settings for {email}', { email: account.email })
  return <Link
    to="/settings"
    search={{ section: 'account' }}
    aria-label={label}
    title={label}
    className={`flex min-w-0 max-w-full items-center gap-2.5 rounded-[9px] py-1.5 text-ink hover:bg-soft focus-visible:outline-2 focus-visible:-outline-offset-2 focus-visible:outline-brand ${collapsed ? 'justify-center px-1' : 'px-1.5'}`}
  >
    <span aria-hidden="true" className="flex size-8 shrink-0 items-center justify-center rounded-full border border-brand/15 bg-soft text-xs font-semibold text-brand">{initials}</span>
    {!collapsed && <span className="min-w-0 text-left">
      <span className="block truncate text-xs font-medium">{t('Heyday account')}</span>
      <span className="block truncate text-[11px] text-muted">{account.email}</span>
    </span>}
  </Link>
}
