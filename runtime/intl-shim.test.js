/**
 * Tests for the en-US `Intl` shim (DateTimeFormat, NumberFormat,
 * RelativeTimeFormat). Fixtures are built with the local `new Date(y, m, d,
 * …)` form and asserted via local accessors so expectations are deterministic
 * regardless of the host/Boa timezone.
 */

import { describe, it, expect } from 'zero/test';

describe('Intl.DateTimeFormat', () => {
  /** A fixed local instant: Friday, 2024-01-05 15:07:09. */
  const d = new Date(2024, 0, 5, 15, 7, 9);

  it('defaults to numeric month/day/year', () => {
    expect(new Intl.DateTimeFormat('en-US').format(d)).toBe('1/5/2024');
  });

  it('formats the MMM D, h:mm A friction trigger', () => {
    const out = new Intl.DateTimeFormat('en-US', {
      month: 'short', day: 'numeric', hour: 'numeric', minute: '2-digit', hour12: true,
    }).format(d);
    expect(out).toBe('Jan 5, 3:07 PM');
  });

  it('renders 2-digit year/month/day', () => {
    const out = new Intl.DateTimeFormat('en-US', {
      year: '2-digit', month: '2-digit', day: '2-digit',
    }).format(d);
    expect(out).toBe('01/05/24');
  });

  it('renders narrow month alone', () => {
    expect(new Intl.DateTimeFormat('en-US', { month: 'narrow' }).format(d)).toBe('J');
  });

  it('renders each dateStyle', () => {
    const ds = (style) => new Intl.DateTimeFormat('en-US', { dateStyle: style }).format(d);
    expect(ds('full')).toBe('Friday, January 5, 2024');
    expect(ds('long')).toBe('January 5, 2024');
    expect(ds('medium')).toBe('Jan 5, 2024');
    expect(ds('short')).toBe('1/5/24');
  });

  it('renders timeStyle short and medium', () => {
    expect(new Intl.DateTimeFormat('en-US', { timeStyle: 'short' }).format(d)).toBe('3:07 PM');
    expect(new Intl.DateTimeFormat('en-US', { timeStyle: 'medium' }).format(d)).toBe('3:07:09 PM');
  });

  it('renders 24-hour time with hour12:false', () => {
    const out = new Intl.DateTimeFormat('en-US', {
      hour: 'numeric', minute: '2-digit', hour12: false,
    }).format(d);
    expect(out).toBe('15:07');
  });

  it('renders weekday long and short', () => {
    expect(new Intl.DateTimeFormat('en-US', { weekday: 'long' }).format(d)).toBe('Friday');
    expect(new Intl.DateTimeFormat('en-US', { weekday: 'short' }).format(d)).toBe('Fri');
  });

  it('joins dateStyle and timeStyle with a comma', () => {
    const out = new Intl.DateTimeFormat('en-US', {
      dateStyle: 'medium', timeStyle: 'short',
    }).format(d);
    expect(out).toBe('Jan 5, 2024, 3:07 PM');
  });

  it('accepts a timestamp number and undefined', () => {
    const fmt = new Intl.DateTimeFormat('en-US');
    expect(fmt.format(d.getTime())).toBe('1/5/2024');
    expect(typeof fmt.format()).toBe('string');
  });

  it('ignores a non-en-US locale without throwing', () => {
    const opts = { month: 'short', day: 'numeric', hour: 'numeric', minute: '2-digit' };
    const en = new Intl.DateTimeFormat('en-US', opts).format(d);
    const fr = new Intl.DateTimeFormat('fr-FR', opts).format(d);
    expect(fr).toBe(en);
  });

  it('reports en-US from resolvedOptions', () => {
    expect(new Intl.DateTimeFormat('fr-FR').resolvedOptions().locale).toBe('en-US');
  });
});

describe('Intl.NumberFormat', () => {
  it('formats a decimal with grouping', () => {
    expect(new Intl.NumberFormat('en-US').format(1234.5)).toBe('1,234.5');
  });

  it('drops grouping with useGrouping:false', () => {
    expect(new Intl.NumberFormat('en-US', { useGrouping: false }).format(1234.5)).toBe('1234.5');
  });

  it('formats USD currency with two fraction digits', () => {
    const usd = new Intl.NumberFormat('en-US', { style: 'currency', currency: 'USD' });
    expect(usd.format(1234.5)).toBe('$1,234.50');
    expect(usd.format(0)).toBe('$0.00');
  });

  it('formats JPY currency with no fraction digits', () => {
    const jpy = new Intl.NumberFormat('en-US', { style: 'currency', currency: 'JPY' });
    expect(jpy.format(1234)).toBe('¥1,234');
  });

  it('formats a percent honoring maximumFractionDigits', () => {
    const pct = new Intl.NumberFormat('en-US', { style: 'percent', maximumFractionDigits: 1 });
    expect(pct.format(0.126)).toBe('12.6%');
    expect(new Intl.NumberFormat('en-US', { style: 'percent' }).format(0.5)).toBe('50%');
  });

  it('honors minimumFractionDigits on a decimal', () => {
    expect(new Intl.NumberFormat('en-US', { minimumFractionDigits: 2 }).format(1)).toBe('1.00');
  });

  it('ignores a non-en-US locale without throwing', () => {
    expect(new Intl.NumberFormat('de-DE').format(1234.5)).toBe('1,234.5');
  });

  it('reports en-US from resolvedOptions', () => {
    expect(new Intl.NumberFormat('de-DE').resolvedOptions().locale).toBe('en-US');
  });
});

describe('Intl.RelativeTimeFormat', () => {
  it('formats past and future in the default always/long style', () => {
    const rtf = new Intl.RelativeTimeFormat('en-US');
    expect(rtf.format(-1, 'day')).toBe('1 day ago');
    expect(rtf.format(3, 'day')).toBe('in 3 days');
    expect(rtf.format(-2, 'hour')).toBe('2 hours ago');
  });

  it('substitutes words with numeric:auto', () => {
    const rtf = new Intl.RelativeTimeFormat('en-US', { numeric: 'auto' });
    expect(rtf.format(-1, 'day')).toBe('yesterday');
    expect(rtf.format(1, 'day')).toBe('tomorrow');
    expect(rtf.format(0, 'day')).toBe('today');
    expect(rtf.format(-1, 'week')).toBe('last week');
  });

  it('falls back to numeric form for auto values without a word', () => {
    const rtf = new Intl.RelativeTimeFormat('en-US', { numeric: 'auto' });
    expect(rtf.format(-3, 'day')).toBe('3 days ago');
  });

  it('pluralizes at the |value| === 1 boundary', () => {
    const rtf = new Intl.RelativeTimeFormat('en-US');
    expect(rtf.format(1, 'day')).toBe('in 1 day');
    expect(rtf.format(2, 'day')).toBe('in 2 days');
  });

  it('accepts plural unit names', () => {
    expect(new Intl.RelativeTimeFormat('en-US').format(-2, 'days')).toBe('2 days ago');
  });

  it('ignores a non-en-US locale without throwing', () => {
    expect(new Intl.RelativeTimeFormat('es-ES').format(-1, 'day')).toBe('1 day ago');
  });

  it('reports en-US from resolvedOptions', () => {
    expect(new Intl.RelativeTimeFormat('es-ES').resolvedOptions().locale).toBe('en-US');
  });
});
