// Use local calendar dates, with an exclusive next-start boundary.
export function currentPeriod(day: number, today = new Date()) {
  if (!Number.isInteger(day) || day < 1 || day > 31) throw new Error('Invalid period start day')
  const boundary = (year: number, month: number) =>
    new Date(year, month, Math.min(day, new Date(year, month + 1, 0).getDate()))
  let month = today.getMonth()
  const year = today.getFullYear()
  if (today < boundary(year, month)) month -= 1
  return { start: boundary(year, month), end: boundary(year, month + 1) }
}

export function periodLabel(day: number, today = new Date()) {
  const { start, end } = currentPeriod(day, today)
  const lastDay = new Date(end.getFullYear(), end.getMonth(), end.getDate() - 1)
  const format = new Intl.DateTimeFormat(undefined, { month: 'short', day: 'numeric', year: 'numeric' })
  return `${format.format(start)} – ${format.format(lastDay)}`
}
