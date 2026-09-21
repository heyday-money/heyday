import { expect, test } from '@playwright/test'
import { currentPeriod } from '../src/lib/period'

test('payday boundaries clamp independently across short months and year changes', () => {
  const cases = [
    [1, [2026, 8, 20], [2026, 8, 1], [2026, 9, 1]],
    [25, [2026, 8, 24], [2026, 7, 25], [2026, 8, 25]],
    [25, [2026, 8, 25], [2026, 8, 25], [2026, 9, 25]],
    [31, [2026, 1, 28], [2026, 1, 28], [2026, 2, 31]],
    [30, [2026, 2, 1], [2026, 1, 28], [2026, 2, 30]],
    [29, [2024, 1, 29], [2024, 1, 29], [2024, 2, 29]],
    [31, [2026, 0, 1], [2025, 11, 31], [2026, 0, 31]],
  ] as const
  for (const [day, today, start, end] of cases) {
    const period = currentPeriod(day, new Date(today[0], today[1], today[2]))
    expect(period.start).toEqual(new Date(start[0], start[1], start[2]))
    expect(period.end).toEqual(new Date(end[0], end[1], end[2]))
  }
})
