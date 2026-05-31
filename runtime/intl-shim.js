/**
 * `Intl` shim — en-US only (DateTimeFormat, NumberFormat, RelativeTimeFormat).
 *
 * Concatenated into `ZERO_DOM_SHIM_BODY` by `crates/zero-runtime/build.rs`
 * and evaluated as a script by the test harness before user modules run.
 * No `import` / `export`; relies on globals installed by `dom-shim.js`.
 *
 * By design this is en-US only: any `locales` argument is accepted and
 * ignored, always producing en-US output. It never throws on an unsupported
 * locale — a documented, deliberate departure from the "clear error"
 * discipline of the other shims.
 *
 * The three constructors are plain `function`s rather than `class`es on
 * purpose: under Boa 0.21.1 a `class` object reachable as a GC root from
 * `globalThis` tips the engine's buggy mid-run collector into a "subtract
 * with overflow" / MapLock `BorrowMutError` panic during the heavy showcase
 * component tests (see the `boa_maplock_finalizer` note). A `function`
 * constructor as a global root does not. Keep them as functions.
 */

/** @type {string[]} */
const MONTHS_LONG = [
  'January', 'February', 'March', 'April', 'May', 'June',
  'July', 'August', 'September', 'October', 'November', 'December',
];
/** @type {string[]} */
const MONTHS_SHORT = [
  'Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun',
  'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec',
];
/** @type {string[]} */
const MONTHS_NARROW = ['J', 'F', 'M', 'A', 'M', 'J', 'J', 'A', 'S', 'O', 'N', 'D'];
/** @type {string[]} */
const WEEKDAYS_LONG = [
  'Sunday', 'Monday', 'Tuesday', 'Wednesday', 'Thursday', 'Friday', 'Saturday',
];
/** @type {string[]} */
const WEEKDAYS_SHORT = ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'];
/** @type {string[]} */
const WEEKDAYS_NARROW = ['S', 'M', 'T', 'W', 'T', 'F', 'S'];

/**
 * Zero-pad an integer to two digits.
 * @param {number} n
 * @returns {string}
 */
function _pad2(n) {
  return n < 10 ? `0${n}` : String(n);
}

/**
 * Coerce a `format` argument to a `Date`. `undefined` → now; a number is read
 * as a timestamp.
 * @param {Date | number} [date]
 * @returns {Date}
 */
function _coerceDate(date) {
  if (date === undefined) return new Date();
  if (date instanceof Date) return date;
  return new Date(date);
}

/**
 * Expand a `dateStyle` preset into en-US component options.
 * @param {string} style
 * @returns {Record<string, string>}
 */
function _expandDateStyle(style) {
  if (style === 'full') {
    return { weekday: 'long', year: 'numeric', month: 'long', day: 'numeric' };
  }
  if (style === 'long') return { year: 'numeric', month: 'long', day: 'numeric' };
  if (style === 'medium') return { year: 'numeric', month: 'short', day: 'numeric' };
  return { year: '2-digit', month: 'numeric', day: 'numeric' };
}

/**
 * Expand a `timeStyle` preset into en-US component options. `full`/`long`
 * include a timezone name the shim cannot reliably produce, so they render as
 * `medium` (documented limitation).
 * @param {string} style
 * @returns {Record<string, string>}
 */
function _expandTimeStyle(style) {
  if (style === 'short') return { hour: 'numeric', minute: '2-digit' };
  return { hour: 'numeric', minute: '2-digit', second: '2-digit' };
}

/**
 * Resolve user options into the component set used for formatting plus the
 * object reported by `resolvedOptions`.
 * @param {Record<string, any>} options
 * @returns {{ components: Record<string, any>, resolved: Record<string, any> }}
 */
function _resolveDateTimeOptions(options) {
  const components = /** @type {Record<string, any>} */ ({});
  if (options.dateStyle) Object.assign(components, _expandDateStyle(options.dateStyle));
  if (options.timeStyle) Object.assign(components, _expandTimeStyle(options.timeStyle));
  if (!options.dateStyle && !options.timeStyle) {
    const keys = ['weekday', 'year', 'month', 'day', 'hour', 'minute', 'second'];
    for (const k of keys) if (options[k] !== undefined) components[k] = options[k];
    if (Object.keys(components).length === 0) {
      components.year = 'numeric';
      components.month = 'numeric';
      components.day = 'numeric';
    }
  }
  if (options.hour12 !== undefined) components.hour12 = options.hour12;
  const resolved = Object.assign(
    { locale: 'en-US', calendar: 'gregory', numberingSystem: 'latn' },
    components,
  );
  return { components, resolved };
}

/**
 * Render the weekday prefix (`''` when no `weekday` option is set).
 * @param {Date} d
 * @param {Record<string, any>} c
 * @returns {string}
 */
function _formatWeekday(d, c) {
  if (!c.weekday) return '';
  const i = d.getDay();
  if (c.weekday === 'narrow') return WEEKDAYS_NARROW[i];
  if (c.weekday === 'short') return WEEKDAYS_SHORT[i];
  return WEEKDAYS_LONG[i];
}

/**
 * Render a textual month (`long`/`short`/`narrow`).
 * @param {number} monthIndex
 * @param {string} style
 * @returns {string}
 */
function _monthName(monthIndex, style) {
  if (style === 'narrow') return MONTHS_NARROW[monthIndex];
  if (style === 'short') return MONTHS_SHORT[monthIndex];
  return MONTHS_LONG[monthIndex];
}

/**
 * Render a numeric year per the `year` option.
 * @param {Date} d
 * @param {string} style
 * @returns {string}
 */
function _yearStr(d, style) {
  const y = d.getFullYear();
  return style === '2-digit' ? _pad2(y % 100) : String(y);
}

/**
 * Render the date portion for a textual month: `Month D, YYYY` and variants.
 * @param {Date} d
 * @param {Record<string, any>} c
 * @returns {string}
 */
function _formatTextualDate(d, c) {
  let s = _monthName(d.getMonth(), c.month);
  const hasDay = c.day !== undefined;
  const hasYear = c.year !== undefined;
  if (hasDay && hasYear) s += ` ${d.getDate()}, ${_yearStr(d, c.year)}`;
  else if (hasDay) s += ` ${d.getDate()}`;
  else if (hasYear) s += ` ${_yearStr(d, c.year)}`;
  return s;
}

/**
 * Render the date portion for a numeric month: slash-separated `M/D/YYYY`.
 * @param {Date} d
 * @param {Record<string, any>} c
 * @returns {string}
 */
function _formatNumericDate(d, c) {
  const parts = /** @type {string[]} */ ([]);
  if (c.month !== undefined) {
    const m = d.getMonth() + 1;
    parts.push(c.month === '2-digit' ? _pad2(m) : String(m));
  }
  if (c.day !== undefined) {
    parts.push(c.day === '2-digit' ? _pad2(d.getDate()) : String(d.getDate()));
  }
  if (c.year !== undefined) parts.push(_yearStr(d, c.year));
  return parts.join('/');
}

/**
 * Render the date portion (`''` when no date component is present).
 * @param {Date} d
 * @param {Record<string, any>} c
 * @returns {string}
 */
function _formatDatePortion(d, c) {
  if (c.month === undefined && c.day === undefined && c.year === undefined) return '';
  const textual = c.month === 'long' || c.month === 'short' || c.month === 'narrow';
  return textual ? _formatTextualDate(d, c) : _formatNumericDate(d, c);
}

/**
 * Render the time portion (`''` when no time component is present).
 * @param {Date} d
 * @param {Record<string, any>} c
 * @returns {string}
 */
function _formatTimePortion(d, c) {
  if (c.hour === undefined && c.minute === undefined && c.second === undefined) return '';
  const hour12 = c.hour12 !== false;
  const h = d.getHours();
  let core = '';
  if (c.hour !== undefined) {
    const hh = hour12 ? (h % 12 === 0 ? 12 : h % 12) : h;
    core = c.hour === '2-digit' ? _pad2(hh) : String(hh);
  }
  if (c.minute !== undefined) core += `${core ? ':' : ''}${_pad2(d.getMinutes())}`;
  if (c.second !== undefined) core += `${core ? ':' : ''}${_pad2(d.getSeconds())}`;
  if (hour12 && c.hour !== undefined) core += h < 12 ? ' AM' : ' PM';
  return core;
}

/**
 * Join the weekday, date, and time portions into the final en-US string.
 * @param {string} weekday
 * @param {string} datePortion
 * @param {string} timePortion
 * @returns {string}
 */
function _assembleDateTime(weekday, datePortion, timePortion) {
  let core = datePortion;
  if (datePortion && timePortion) core = `${datePortion}, ${timePortion}`;
  else if (timePortion) core = timePortion;
  if (weekday) return core ? `${weekday}, ${core}` : weekday;
  return core;
}

/**
 * en-US `Intl.DateTimeFormat`. A `function` constructor (not a `class`) — see
 * the file header for the Boa GC rationale.
 * @constructor
 * @param {string | string[]} [locales]
 * @param {Record<string, any>} [options]
 */
function DateTimeFormat(locales, options) {
  void locales;
  const resolved = _resolveDateTimeOptions(options ?? {});
  /** @type {Record<string, any>} */
  this._components = resolved.components;
  /** @type {Record<string, any>} */
  this._resolved = resolved.resolved;
}

/**
 * @param {Date | number} [date]
 * @returns {string}
 */
DateTimeFormat.prototype.format = function format(date) {
  const d = _coerceDate(date);
  const c = this._components;
  return _assembleDateTime(
    _formatWeekday(d, c),
    _formatDatePortion(d, c),
    _formatTimePortion(d, c),
  );
};

/**
 * @returns {Record<string, any>}
 */
DateTimeFormat.prototype.resolvedOptions = function resolvedOptions() {
  return Object.assign({}, this._resolved);
};

/** @type {Record<string, string>} */
const CURRENCY_SYMBOLS = { USD: '$', EUR: '€', GBP: '£', JPY: '¥' };

/**
 * Group an unsigned integer-digit string with `,` thousands separators.
 * @param {string} intStr
 * @returns {string}
 */
function _groupInteger(intStr) {
  let out = '';
  for (let i = 0; i < intStr.length; i++) {
    if (i > 0 && (intStr.length - i) % 3 === 0) out += ',';
    out += intStr[i];
  }
  return out;
}

/**
 * Render a number with fixed fraction-digit bounds and optional grouping.
 * Rounds via `toFixed` (half-up), then trims trailing zeros down to `minFrac`.
 * @param {number} value
 * @param {number} minFrac
 * @param {number} maxFrac
 * @param {boolean} useGrouping
 * @returns {string}
 */
function _formatNumber(value, minFrac, maxFrac, useGrouping) {
  const neg = value < 0;
  const fixed = Math.abs(value).toFixed(maxFrac);
  let [intPart, frac = ''] = fixed.split('.');
  if (frac.length > minFrac) {
    let end = frac.length;
    while (end > minFrac && frac[end - 1] === '0') end -= 1;
    frac = frac.slice(0, end);
  }
  if (useGrouping) intPart = _groupInteger(intPart);
  const body = frac ? `${intPart}.${frac}` : intPart;
  return neg ? `-${body}` : body;
}

/**
 * Resolve the fraction-digit bounds for the given style and user overrides.
 * @param {Record<string, any>} options
 * @param {number} defMin
 * @param {number} defMax
 * @returns {{ min: number, max: number }}
 */
function _resolveFractionDigits(options, defMin, defMax) {
  const min = options.minimumFractionDigits !== undefined
    ? options.minimumFractionDigits : defMin;
  let max = options.maximumFractionDigits !== undefined
    ? options.maximumFractionDigits : Math.max(defMax, min);
  if (max < min) max = min;
  return { min, max };
}

/**
 * @param {number} value
 * @param {Record<string, any>} o
 * @returns {string}
 */
function _formatDecimal(value, o) {
  const { min, max } = _resolveFractionDigits(o, 0, 3);
  return _formatNumber(value, min, max, o.useGrouping !== false);
}

/**
 * @param {number} value
 * @param {Record<string, any>} o
 * @returns {string}
 */
function _formatCurrency(value, o) {
  const code = String(o.currency || '');
  const symbol = CURRENCY_SYMBOLS[code] || `${code} `;
  const defFrac = code === 'JPY' ? 0 : 2;
  const { min, max } = _resolveFractionDigits(o, defFrac, defFrac);
  return symbol + _formatNumber(value, min, max, o.useGrouping !== false);
}

/**
 * @param {number} value
 * @param {Record<string, any>} o
 * @returns {string}
 */
function _formatPercent(value, o) {
  const { min, max } = _resolveFractionDigits(o, 0, 0);
  return `${_formatNumber(value * 100, min, max, o.useGrouping !== false)}%`;
}

/**
 * en-US `Intl.NumberFormat`. A `function` constructor (not a `class`) — see
 * the file header for the Boa GC rationale.
 * @constructor
 * @param {string | string[]} [locales]
 * @param {Record<string, any>} [options]
 */
function NumberFormat(locales, options) {
  void locales;
  /** @type {Record<string, any>} */
  this._opts = Object.assign({}, options ?? {});
  if (!this._opts.style) this._opts.style = 'decimal';
}

/**
 * @param {number} value
 * @returns {string}
 */
NumberFormat.prototype.format = function format(value) {
  const v = Number(value);
  const o = this._opts;
  if (o.style === 'currency') return _formatCurrency(v, o);
  if (o.style === 'percent') return _formatPercent(v, o);
  return _formatDecimal(v, o);
};

/**
 * @returns {Record<string, any>}
 */
NumberFormat.prototype.resolvedOptions = function resolvedOptions() {
  return Object.assign({ locale: 'en-US', numberingSystem: 'latn' }, this._opts);
};

/**
 * Normalize a unit to its singular form (`days` → `day`).
 * @param {string} unit
 * @returns {string}
 */
function _normalizeUnit(unit) {
  const u = String(unit);
  return u.endsWith('s') ? u.slice(0, -1) : u;
}

/**
 * Render the numeric (`always`) en-US form: `N unit(s) ago` / `in N unit(s)`.
 * @param {number} value
 * @param {string} unit
 * @returns {string}
 */
function _formatRelativeAlways(value, unit) {
  const abs = Math.abs(value);
  const word = abs === 1 ? unit : `${unit}s`;
  return value < 0 ? `${abs} ${word} ago` : `in ${abs} ${word}`;
}

/**
 * Render the `auto` en-US word substitutions, falling back to the numeric
 * form when no word exists for the value/unit pair.
 * @param {number} value
 * @param {string} unit
 * @returns {string}
 */
function _formatRelativeAuto(value, unit) {
  if (unit === 'day') {
    if (value === -1) return 'yesterday';
    if (value === 0) return 'today';
    if (value === 1) return 'tomorrow';
  } else if (unit === 'week' || unit === 'month' || unit === 'quarter' || unit === 'year') {
    if (value === -1) return `last ${unit}`;
    if (value === 0) return `this ${unit}`;
    if (value === 1) return `next ${unit}`;
  } else if (unit === 'hour' || unit === 'minute') {
    if (value === 0) return `this ${unit}`;
  } else if (unit === 'second' && value === 0) {
    return 'now';
  }
  return _formatRelativeAlways(value, unit);
}

/**
 * en-US `Intl.RelativeTimeFormat`. A `function` constructor (not a `class`) —
 * see the file header for the Boa GC rationale.
 * @constructor
 * @param {string | string[]} [locales]
 * @param {Record<string, any>} [options]
 */
function RelativeTimeFormat(locales, options) {
  void locales;
  const o = options ?? {};
  /** @type {string} */
  this._numeric = o.numeric === 'auto' ? 'auto' : 'always';
  /** @type {string} */
  this._style = o.style || 'long';
}

/**
 * @param {number} value
 * @param {string} unit
 * @returns {string}
 */
RelativeTimeFormat.prototype.format = function format(value, unit) {
  const u = _normalizeUnit(unit);
  if (this._numeric === 'auto') return _formatRelativeAuto(value, u);
  return _formatRelativeAlways(value, u);
};

/**
 * @returns {Record<string, any>}
 */
RelativeTimeFormat.prototype.resolvedOptions = function resolvedOptions() {
  return {
    locale: 'en-US',
    numberingSystem: 'latn',
    numeric: this._numeric,
    style: this._style,
  };
};

if (typeof globalThis.Intl === 'undefined') {
  Object.defineProperty(globalThis, 'Intl', {
    value: { DateTimeFormat, NumberFormat, RelativeTimeFormat },
    writable: true,
    configurable: true,
  });
}
