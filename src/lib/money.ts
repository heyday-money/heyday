export const currencies = ['THB', 'USD', 'EUR', 'GBP', 'JPY', 'CNY', 'SGD', 'AUD', 'CAD', 'CHF', 'INR', 'KRW', 'VND', 'KWD', 'BHD'] as const

export function fractionDigits(currency: string) {
  return new Intl.NumberFormat('en', { style: 'currency', currency }).resolvedOptions().maximumFractionDigits ?? 2
}

export function decimalToInteger(value: string, digits: number): string {
  const match = /^(-?)(\d+)(?:\.(\d+))?$/.exec(value.trim())
  if (!match || (match[3]?.length ?? 0) > digits) throw new Error(`Enter a valid amount with up to ${digits} decimal places.`)
  const integer = BigInt(`${match[1]}${match[2]}${(match[3] ?? '').padEnd(digits, '0')}`)
  if (integer < -9223372036854775808n || integer > 9223372036854775807n) throw new Error('Amount is outside the supported range.')
  return integer.toString()
}

export function formatAmount(value: string, currency: string) {
  const digits = fractionDigits(currency)
  const amount = BigInt(value)
  const absolute = amount < 0n ? -amount : amount
  const scale = 10n ** BigInt(digits)
  const whole = new Intl.NumberFormat(undefined, { maximumFractionDigits: 0 }).format(absolute / scale)
  const decimal = new Intl.NumberFormat().formatToParts(1.1).find(part => part.type === 'decimal')?.value ?? '.'
  return `${amount < 0n ? '−' : ''}${whole}${digits ? decimal + (absolute % scale).toString().padStart(digits, '0') : ''} ${currency}`
}
