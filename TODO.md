# TODO

## Payroll loan deductions and repayment recording

- [x] Give linked salary deductions priority in Outlook to prevent counting the same loan payment twice.
  - Keep the account's normal monthly installment saved. Forecast only the remaining installment after linked payroll deductions, with a minimum of zero.
  - Use the effective deduction for each payday cycle, including explicit overrides and salary recurrence/availability. Preserve explicitly entered additional payments outside payroll.
  - Apply the same rule to forecast totals and closing-cash carry-forward. Explain the payroll-covered amount and remaining payment in the interface.
  - Cover GHB Home Loan (฿9,200) and Student Loan (฿1,260) with equivalent synthetic test fixtures: matching deductions must eliminate the duplicate ฿10,460 outflow. Test partial coverage, excess coverage, zero overrides, and inactive salaries without seeding personal data.

- [x] Add a confirmed monthly **Record salary payment** flow with linked payroll loan repayments.
  - Record the net salary deposited into the bank and a linked breakdown of payroll deductions and loan repayments atomically.
  - Split each loan repayment into principal, interest, and fees. Only principal reduces the outstanding loan balance; payroll repayments must not debit the bank again because the salary deposit is already net.
  - Show the linked breakdown in salary and loan transaction history so users can review payments each month and reconcile outstanding principal against lender statements.
  - Prevent accidental duplicate recording of the same salary occurrence; define safe handling of existing salary receipts and grouped reversal/correction.
  - Keep forecast definitions separate from actual transactions. Require monthly confirmation; do not infer that a forecast payment occurred.
  - Validate exact monetary amounts in Rust, protect drafts, and include English/Thai wording and regression coverage for balances, history, reconciliation, and reversal.

## Verification (2026-10-07)

- 125 Rust tests passed, including payroll amounts and rollback, duplicate receipts,
  exact large values, contract principal, loan reconciliation, grouped reversal,
  interest-only/zero-net history, and beta.1 backup upgrades.
- Targeted Playwright checks passed for payroll recording/history, forecasts and
  carry-forward, actual cashflow, income, Home, transaction history, reconciliation,
  and English/Thai catalog coverage. Salary dialog layout reviewed in the browser.
- Production frontend build and `git diff --check` passed. The existing bundle-size
  warning remains. Packaged native-app smoke testing remains a release check.
- Implementation notes: [Confirmed salary payments](docs/development.md#confirmed-salary-payments-beta2).
