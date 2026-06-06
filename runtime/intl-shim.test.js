/**
 * Tests for the en-US `Intl` shim (DateTimeFormat, NumberFormat,
 * RelativeTimeFormat). Fixtures are built with the local `new Date(y, m, d,
 * …)` form and asserted via local accessors so expectations are deterministic
 * regardless of the host timezone; UTC cases build from `Date.UTC` instead.
 */

import { describe, it, expect } from 'zero/test';

/**
 * Assert that `fn` throws an error that is an `instanceof ctor` and whose
 * message contains `msgPart`.
 * @param {() => void} fn
 * @param {Function} ctor
 * @param {string} [msgPart]
 * @returns {void}
 */
function expectThrows(fn, ctor, msgPart) {
  let caught = null;
  try {
    fn();
  } catch (e) {
    caught = e;
  }
  expect(caught === null).toBe(false);
  expect(caught instanceof ctor).toBe(true);
  if (msgPart !== undefined) expect(String(caught.message)).toContain(msgPart);
}

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

  it('throws RangeError on an invalid component value (friction probe)', () => {
    expectThrows(
      () => new Intl.DateTimeFormat('en-US', { month: 'short', day: '', hour: 'numeric' }),
      RangeError,
      'out of range',
    );
  });

  it('throws RangeError on an invalid dateStyle or timeStyle', () => {
    expectThrows(() => new Intl.DateTimeFormat('en-US', { dateStyle: 'huge' }), RangeError, 'dateStyle');
    expectThrows(() => new Intl.DateTimeFormat('en-US', { timeStyle: '' }), RangeError, 'timeStyle');
  });

  it('throws RangeError when hour12 is not a boolean', () => {
    expectThrows(
      () => new Intl.DateTimeFormat('en-US', { hour: 'numeric', hour12: 'yes' }),
      RangeError,
      'hour12',
    );
  });

  it('throws a clear shim error on spec-valid but unimplemented options', () => {
    expectThrows(
      () => new Intl.DateTimeFormat('en-US', { hour: 'numeric', timeZoneName: 'short' }),
      Error,
      'is not implemented',
    );
    expectThrows(
      () => new Intl.DateTimeFormat('en-US', { localeMatcher: 'lookup' }),
      Error,
      'localeMatcher',
    );
  });

  it('throws RangeError per invalid component family', () => {
    expectThrows(() => new Intl.DateTimeFormat('en-US', { weekday: 'tiny' }), RangeError, 'weekday');
    expectThrows(() => new Intl.DateTimeFormat('en-US', { year: 'long' }), RangeError, 'year');
    expectThrows(() => new Intl.DateTimeFormat('en-US', { month: 'bogus' }), RangeError, 'month');
    expectThrows(() => new Intl.DateTimeFormat('en-US', { hour: 'full' }), RangeError, 'hour');
  });

  it('ignores truly unknown option keys, as browsers do', () => {
    expect(new Intl.DateTimeFormat('en-US', { foo: 1 }).format(d)).toBe('1/5/2024');
  });

  it('defaults hour-only formats to 12-hour when hour12 is absent', () => {
    expect(new Intl.DateTimeFormat('en-US', { hour: 'numeric' }).format(d)).toBe('3 PM');
  });

  it('renders via UTC accessors with timeZone:"UTC"', () => {
    const utc = new Date(Date.UTC(2024, 0, 5, 23, 7, 9));
    const out = new Intl.DateTimeFormat('en-US', {
      timeZone: 'UTC', month: 'short', day: 'numeric', hour: 'numeric', minute: '2-digit',
    }).format(utc);
    expect(out).toBe('Jan 5, 11:07 PM');
    // Date rollover: host-local rendering of this instant may be Jan 4 or 5;
    // UTC must always read Jan 5.
    const rollover = new Date(Date.UTC(2024, 0, 5, 0, 30));
    const day = new Intl.DateTimeFormat('en-US', { timeZone: 'UTC', day: 'numeric' }).format(rollover);
    expect(day).toBe('5');
  });

  it('accepts lowercase "utc" as equivalent to "UTC"', () => {
    const utc = new Date(Date.UTC(2024, 0, 5, 23, 7, 9));
    const upper = new Intl.DateTimeFormat('en-US', { timeZone: 'UTC', hour: 'numeric' }).format(utc);
    const lower = new Intl.DateTimeFormat('en-US', { timeZone: 'utc', hour: 'numeric' }).format(utc);
    expect(lower).toBe(upper);
  });

  it('throws a clear shim error for any non-UTC timeZone', () => {
    expectThrows(
      () => new Intl.DateTimeFormat('en-US', { timeZone: 'America/New_York' }),
      Error,
      'is not implemented',
    );
  });

  it('reports timeZone from resolvedOptions only when passed', () => {
    const withTz = new Intl.DateTimeFormat('en-US', { timeZone: 'utc' }).resolvedOptions();
    expect(withTz.timeZone).toBe('UTC');
    const withoutTz = new Intl.DateTimeFormat('en-US').resolvedOptions();
    expect('timeZone' in withoutTz).toBe(false);
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

  it('throws RangeError on an invalid or unshimmed style', () => {
    expectThrows(() => new Intl.NumberFormat('en-US', { style: 'unit' }), RangeError, 'style');
    expectThrows(() => new Intl.NumberFormat('en-US', { style: '' }), RangeError, 'style');
  });

  it('enforces the currency code contract', () => {
    expectThrows(() => new Intl.NumberFormat('en-US', { style: 'currency' }), TypeError, 'required');
    expectThrows(
      () => new Intl.NumberFormat('en-US', { style: 'currency', currency: 'DOLLARS' }),
      RangeError,
      'Invalid currency code',
    );
    expectThrows(
      () => new Intl.NumberFormat('en-US', { style: 'currency', currency: 'U$' }),
      RangeError,
      'Invalid currency code',
    );
    const usd = new Intl.NumberFormat('en-US', { style: 'currency', currency: 'usd' });
    expect(usd.format(1)).toBe('$1.00');
    expect(usd.resolvedOptions().currency).toBe('USD');
  });

  it('range-checks fraction-digit options', () => {
    expectThrows(
      () => new Intl.NumberFormat('en-US', { minimumFractionDigits: -1 }),
      RangeError,
      'minimumFractionDigits',
    );
    expectThrows(
      () => new Intl.NumberFormat('en-US', { maximumFractionDigits: 101 }),
      RangeError,
      'maximumFractionDigits',
    );
    expectThrows(
      () => new Intl.NumberFormat('en-US', { maximumFractionDigits: 1.5 }),
      RangeError,
      'maximumFractionDigits',
    );
  });

  it('throws RangeError when explicit min exceeds explicit max', () => {
    expectThrows(
      () => new Intl.NumberFormat('en-US', { minimumFractionDigits: 4, maximumFractionDigits: 2 }),
      RangeError,
      'maximumFractionDigits',
    );
  });

  it('floats the default max up under an explicit min alone', () => {
    expect(new Intl.NumberFormat('en-US', { minimumFractionDigits: 4 }).format(1)).toBe('1.0000');
  });

  it('throws RangeError when useGrouping is not a boolean', () => {
    expectThrows(() => new Intl.NumberFormat('en-US', { useGrouping: 'no' }), RangeError, 'useGrouping');
  });

  it('throws a clear shim error on spec-valid but unimplemented options', () => {
    expectThrows(
      () => new Intl.NumberFormat('en-US', { notation: 'compact' }),
      Error,
      'is not implemented',
    );
    expectThrows(
      () => new Intl.NumberFormat('en-US', { style: 'currency', currency: 'USD', currencyDisplay: 'code' }),
      Error,
      'currencyDisplay',
    );
    expect(
      new Intl.NumberFormat('en-US', {
        style: 'currency', currency: 'USD', currencyDisplay: 'symbol',
      }).format(1),
    ).toBe('$1.00');
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

  it('validates constructor options', () => {
    expectThrows(
      () => new Intl.RelativeTimeFormat('en-US', { numeric: 'sometimes' }),
      RangeError,
      'numeric',
    );
    // Spec-valid styles the shim does not render: shim error, not RangeError.
    expectThrows(() => new Intl.RelativeTimeFormat('en-US', { style: 'short' }), Error, 'is not implemented');
    expectThrows(() => new Intl.RelativeTimeFormat('en-US', { style: 'narrow' }), Error, 'is not implemented');
    // Spec-invalid style: RangeError.
    expectThrows(() => new Intl.RelativeTimeFormat('en-US', { style: 'compact' }), RangeError, 'style');
    expectThrows(
      () => new Intl.RelativeTimeFormat('en-US', { localeMatcher: 'lookup' }),
      Error,
      'localeMatcher',
    );
  });

  it('throws RangeError on an invalid format() unit', () => {
    const rtf = new Intl.RelativeTimeFormat('en-US');
    expectThrows(() => rtf.format(-2, 'bananas'), RangeError, 'bananas');
  });

  it('throws RangeError on a non-finite format() value', () => {
    const rtf = new Intl.RelativeTimeFormat('en-US');
    expectThrows(() => rtf.format(Infinity, 'day'), RangeError, 'finite');
    expectThrows(() => rtf.format(NaN, 'day'), RangeError, 'finite');
  });

  it('still coerces numeric-string values', () => {
    expect(new Intl.RelativeTimeFormat('en-US').format('-1', 'day')).toBe('1 day ago');
  });
});
