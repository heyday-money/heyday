import { Link } from '@tanstack/react-router'
import { History, Pencil } from 'lucide-react'
import { t as translate, useLanguage } from '../lib/i18n'
import type { Account } from '../lib/desktop'
import { Button } from './ui/button'
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from './ui/tooltip'

export function AccountActions({ account, onEdit }: { account: Account; onEdit: (account: Account) => void }) {
  useLanguage()
  return <TooltipProvider><div className="flex shrink-0 items-center gap-1">
    {!account.is_archived && !account.paid_off_on && <Tooltip>
      <TooltipTrigger asChild><Button type="button" variant="ghost" size="icon-sm" aria-label={translate("Edit account {value0}", { value0: account.name })} onClick={() => onEdit(account)}><Pencil aria-hidden="true" /></Button></TooltipTrigger>
      <TooltipContent>{translate("Edit account")}</TooltipContent>
    </Tooltip>}
    <Tooltip>
      <TooltipTrigger asChild><Button asChild variant="ghost" size="icon-sm"><Link to="/transactions" search={{ account: account.id }} aria-label={translate("Transaction history")}><History aria-hidden="true" /></Link></Button></TooltipTrigger>
      <TooltipContent>{translate("Transaction history")}</TooltipContent>
    </Tooltip>
  </div></TooltipProvider>
}
