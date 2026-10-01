import { t as translate, useLanguage } from "../lib/i18n"
import { Link } from '@tanstack/react-router'
import { statementTotals, spendingCategories } from '../lib/card-billing'
import { boundary, addMonths, plannerMoney, type PlannerData } from '../lib/cashflow'
import { dateKey } from '../lib/financial'
export function CardOutlookDetails({data,months}:{data:PlannerData;months:string[]}){
  useLanguage()

 if(data.source_currency!=='THB')return null
 const billing=data.card_billing
 return <details className="mb-4 rounded-xl border border-line bg-card p-4"><summary className="cursor-pointer font-semibold">{translate("Credit card bills & spending details")}</summary><p className="my-3 text-sm">{translate("Bill plans affect cash on the planned payment date. Purchases below are category spending only — excluded from cash outflow totals. Review manual plans for duplicates.")}</p>
 {data.credit_cards.map(card=><details key={card.id} className="border-t border-line py-3"><summary className="cursor-pointer font-medium">{card.name}</summary><Link to="/accounts/$accountId/billing" params={{accountId:card.id}} className="my-2 inline-block text-sm text-brand">{translate("Manage billing & payments")}</Link>
 {billing?.statements.filter(s=>s.account_id===card.id).map(s=>{const t=statementTotals(billing,s);return <div key={s.id} className="mb-3 rounded-lg bg-soft p-3 text-sm"><p className="font-medium">{translate("Bill ending")}{" "}{s.end_date} {" "}{translate("· Due")}{" "}{s.due_date} · {translate(t.status)}</p><p>{translate("Statement remaining")}{" "}{plannerMoney(t.remaining)} {" "}{translate("· Minimum remaining")}{" "}{plannerMoney(t.minimum)}</p><p>{translate("Applied payments")}{" "}{plannerMoney(t.paid)} · {t.plan?translate("{value0} plan on {value1}: {value2} remaining", { value0: t.plan.mode==='full'?translate("Full statement"):t.plan.mode==='minimum'?translate("Minimum"):translate("Partial"), value1: t.plan.date, value2: plannerMoney(t.planned) }):translate("Not planned")}</p>{t.plan&&<p>{translate("After plan:")}{" "}{plannerMoney(t.afterPlan)} {" "}{translate("remaining, excluding future interest and fees.")}{" "}{t.planned<t.minimum?translate("Plan is below the remaining minimum."):''} {t.plan.date>s.due_date?translate("Planned after due date."):''} {t.forecast===0n&&t.planned>0n?translate("Needs review / unavailable: excluded from forecast."):''}</p>}</div>})}
 {!billing?.statements.some(s=>s.account_id===card.id)&&<p className="mb-2 text-sm">{translate("No confirmed bills. Existing recorded-payment and installment estimates apply.")}</p>}
 <details><summary className="cursor-pointer text-sm font-medium">{translate("Spending by category · purchases, not cash payments")}</summary>{months.map(month=>{const start=dateKey(boundary(month,data.period_start_day)),end=dateKey(boundary(addMonths(month,1),data.period_start_day)),categories=spendingCategories(data.card_transactions,card.id,start,end);return <div key={month} className="mt-3 text-sm"><h4 className="font-medium">{month} · {start} {" "}{translate("to")}{" "}{end} {" "}{translate("(exclusive)")}</h4>{categories.map(c=><details key={c.name} className="pl-3"><summary className="cursor-pointer">{c.name}: {plannerMoney(c.amount)}</summary>{c.transactions.map(t=><p key={t.id} className="pl-3 text-xs">{t.date} · {plannerMoney(BigInt(t.amount))}</p>)}</details>)}{!categories.length&&<p className="text-xs text-muted">{translate("No recorded purchases.")}</p>}</div>})}</details>
 </details>)}
 </details>
}
