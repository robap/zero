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
 * Option *values*, by contrast, are validated: an invalid value for a
 * supported option throws a browser-faithful `RangeError`, and a spec-valid
 * option the shim does not implement throws the standard `zero test: … is
 * not implemented` shim error. Truly unknown keys are ignored, as in
 * browsers. This keeps formatter configuration pinnable by tests and makes
 * mutated option literals killable by `zero mutate`.
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
 * Throw a browser-faithful `RangeError` when `value` is defined and not in
 * the allowed set. `undefined` (option not passed) is always accepted.
 * @param {string} ctor - constructor name for the error message
 * @param {string} option - option property name
 * @param {any} value - the user-supplied value
 * @param {string[]} allowed - the spec's allowed values for this option
 * @returns {void}
 */
function _requireOneOf(ctor, option, value, allowed) {
  if (value === undefined) return;
  if (!allowed.includes(value)) {
    throw new RangeError(
      `Value "${value}" out of range for Intl.${ctor} options property ${option}`,
    );
  }
}

/**
 * Throw a `RangeError` when `value` is defined and not an actual boolean.
 * Deliberately stricter than ECMA-402 (which `ToBoolean`-coerces): in a test
 * runtime, `hour12: "yes"` is a bug worth surfacing.
 * @param {string} ctor - constructor name for the error message
 * @param {string} option - option property name
 * @param {any} value - the user-supplied value
 * @returns {void}
 */
function _requireBoolean(ctor, option, value) {
  if (value === undefined) return;
  if (typeof value !== 'boolean') {
    throw new RangeError(
      `Value "${value}" out of range for Intl.${ctor} options property ${option} (expected a boolean)`,
    );
  }
}

/**
 * Throw the standard shim-boundary error for any spec-valid option key the
 * shim does not implement. Loud beats silently-different-from-browser output.
 * @param {string} ctor - constructor name for the error message
 * @param {Record<string, any>} options - the user-supplied options object
 * @param {string[]} keys - the spec-defined keys this shim rejects
 * @returns {void}
 */
function _rejectUnsupported(ctor, options, keys) {
  for (const key of keys) {
    if (options[key] !== undefined) {
      throw new Error(
        `zero test: Intl.${ctor} option "${key}" is not implemented. ` +
        'Remove it or guard the call for the test runtime.',
      );
    }
  }
}

/** @type {string[]} */
const DTF_UNSUPPORTED = [
  'era', 'timeZoneName', 'hourCycle', 'dayPeriod', 'fractionalSecondDigits',
  'calendar', 'numberingSystem', 'formatMatcher', 'localeMatcher',
];

/** @type {Record<string, string[]>} */
const DTF_COMPONENT_VALUES = {
  weekday: ['long', 'short', 'narrow'],
  year: ['numeric', '2-digit'],
  month: ['numeric', '2-digit', 'long', 'short', 'narrow'],
  day: ['numeric', '2-digit'],
  hour: ['numeric', '2-digit'],
  minute: ['numeric', '2-digit'],
  second: ['numeric', '2-digit'],
};

/**
 * Validate `DateTimeFormat` options, throwing on invalid values. Truly
 * unknown keys (not defined by ECMA-402) are ignored, as in browsers.
 * @param {Record<string, any>} options
 * @returns {void}
 */
function _validateDateTimeOptions(options) {
  _rejectUnsupported('DateTimeFormat', options, DTF_UNSUPPORTED);
  for (const key of Object.keys(DTF_COMPONENT_VALUES)) {
    _requireOneOf('DateTimeFormat', key, options[key], DTF_COMPONENT_VALUES[key]);
  }
  const styles = ['full', 'long', 'medium', 'short'];
  _requireOneOf('DateTimeFormat', 'dateStyle', options.dateStyle, styles);
  _requireOneOf('DateTimeFormat', 'timeStyle', options.timeStyle, styles);
  _requireBoolean('DateTimeFormat', 'hour12', options.hour12);
  if (options.timeZone !== undefined && String(options.timeZone).toUpperCase() !== 'UTC') {
    throw new Error(
      `zero test: Intl.DateTimeFormat timeZone "${options.timeZone}" is not ` +
      'implemented (only "UTC" is supported). Use "UTC" or remove the option.',
    );
  }
}

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
  if (options.timeZone !== undefined) components.timeZone = 'UTC';
  const resolved = Object.assign(
    { locale: 'en-US', calendar: 'gregory', numberingSystem: 'latn' },
    components,
  );
  return { components, resolved };
}

/**
 * Extract the date/time components the renderers read, from either the local
 * or the UTC accessor family — the single site where `timeZone: "UTC"`
 * switches rendering.
 * @param {Date} d
 * @param {boolean} utc
 * @returns {{ year: number, monthIndex: number, day: number,
 *   weekdayIndex: number, hours: number, minutes: number, seconds: number }}
 */
function _dateParts(d, utc) {
  if (utc) {
    return {
      year: d.getUTCFullYear(),
      monthIndex: d.getUTCMonth(),
      day: d.getUTCDate(),
      weekdayIndex: d.getUTCDay(),
      hours: d.getUTCHours(),
      minutes: d.getUTCMinutes(),
      seconds: d.getUTCSeconds(),
    };
  }
  return {
    year: d.getFullYear(),
    monthIndex: d.getMonth(),
    day: d.getDate(),
    weekdayIndex: d.getDay(),
    hours: d.getHours(),
    minutes: d.getMinutes(),
    seconds: d.getSeconds(),
  };
}

/**
 * Render the weekday prefix (`''` when no `weekday` option is set).
 * @param {ReturnType<typeof _dateParts>} p
 * @param {Record<string, any>} c
 * @returns {string}
 */
function _formatWeekday(p, c) {
  if (!c.weekday) return '';
  const i = p.weekdayIndex;
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
 * @param {ReturnType<typeof _dateParts>} p
 * @param {string} style
 * @returns {string}
 */
function _yearStr(p, style) {
  const y = p.year;
  return style === '2-digit' ? _pad2(y % 100) : String(y);
}

/**
 * Render the date portion for a textual month: `Month D, YYYY` and variants.
 * @param {ReturnType<typeof _dateParts>} p
 * @param {Record<string, any>} c
 * @returns {string}
 */
function _formatTextualDate(p, c) {
  let s = _monthName(p.monthIndex, c.month);
  const hasDay = c.day !== undefined;
  const hasYear = c.year !== undefined;
  if (hasDay && hasYear) s += ` ${p.day}, ${_yearStr(p, c.year)}`;
  else if (hasDay) s += ` ${p.day}`;
  else if (hasYear) s += ` ${_yearStr(p, c.year)}`;
  return s;
}

/**
 * Render the date portion for a numeric month: slash-separated `M/D/YYYY`.
 * @param {ReturnType<typeof _dateParts>} p
 * @param {Record<string, any>} c
 * @returns {string}
 */
function _formatNumericDate(p, c) {
  const parts = /** @type {string[]} */ ([]);
  if (c.month !== undefined) {
    const m = p.monthIndex + 1;
    parts.push(c.month === '2-digit' ? _pad2(m) : String(m));
  }
  if (c.day !== undefined) {
    parts.push(c.day === '2-digit' ? _pad2(p.day) : String(p.day));
  }
  if (c.year !== undefined) parts.push(_yearStr(p, c.year));
  return parts.join('/');
}

/**
 * Render the date portion (`''` when no date component is present).
 * @param {ReturnType<typeof _dateParts>} p
 * @param {Record<string, any>} c
 * @returns {string}
 */
function _formatDatePortion(p, c) {
  if (c.month === undefined && c.day === undefined && c.year === undefined) return '';
  const textual = c.month === 'long' || c.month === 'short' || c.month === 'narrow';
  return textual ? _formatTextualDate(p, c) : _formatNumericDate(p, c);
}

/**
 * Render the time portion (`''` when no time component is present).
 * @param {ReturnType<typeof _dateParts>} p
 * @param {Record<string, any>} c
 * @returns {string}
 */
function _formatTimePortion(p, c) {
  if (c.hour === undefined && c.minute === undefined && c.second === undefined) return '';
  const hour12 = c.hour12 !== false;
  const h = p.hours;
  let core = '';
  if (c.hour !== undefined) {
    const hh = hour12 ? (h % 12 === 0 ? 12 : h % 12) : h;
    core = c.hour === '2-digit' ? _pad2(hh) : String(hh);
  }
  if (c.minute !== undefined) core += `${core ? ':' : ''}${_pad2(p.minutes)}`;
  if (c.second !== undefined) core += `${core ? ':' : ''}${_pad2(p.seconds)}`;
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
 * en-US `Intl.DateTimeFormat`.
 * @constructor
 * @param {string | string[]} [locales]
 * @param {Record<string, any>} [options]
 */
function DateTimeFormat(locales, options) {
  void locales;
  const o = options ?? {};
  _validateDateTimeOptions(o);
  const resolved = _resolveDateTimeOptions(o);
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
  const c = this._components;
  const p = _dateParts(_coerceDate(date), c.timeZone === 'UTC');
  return _assembleDateTime(
    _formatWeekday(p, c),
    _formatDatePortion(p, c),
    _formatTimePortion(p, c),
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
  const max = options.maximumFractionDigits !== undefined
    ? options.maximumFractionDigits : Math.max(defMax, min);
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
 * Validate one fraction-digit option: when present it must be an integer in
 * the spec's 0–100 range.
 * @param {string} option - option property name
 * @param {any} value - the user-supplied value
 * @returns {void}
 */
function _requireDigitOption(option, value) {
  if (value === undefined) return;
  if (!Number.isInteger(value) || value < 0 || value > 100) {
    throw new RangeError(`${option} value is out of range`);
  }
}

/** @type {string[]} */
const NF_UNSUPPORTED = [
  'notation', 'unit', 'unitDisplay', 'signDisplay', 'compactDisplay',
  'currencySign', 'roundingMode', 'roundingPriority', 'roundingIncrement',
  'trailingZeroDisplay', 'minimumIntegerDigits', 'minimumSignificantDigits',
  'maximumSignificantDigits', 'localeMatcher',
];

/**
 * Validate `NumberFormat` options, throwing on invalid values, and normalize
 * the currency code to uppercase. Truly unknown keys are ignored, as in
 * browsers.
 * @param {Record<string, any>} options
 * @returns {void}
 */
function _validateNumberOptions(options) {
  _rejectUnsupported('NumberFormat', options, NF_UNSUPPORTED);
  _requireOneOf('NumberFormat', 'style', options.style, ['decimal', 'currency', 'percent']);
  _requireBoolean('NumberFormat', 'useGrouping', options.useGrouping);
  if (options.currencyDisplay !== undefined && options.currencyDisplay !== 'symbol') {
    throw new Error(
      `zero test: Intl.NumberFormat option "currencyDisplay" value ` +
      `"${options.currencyDisplay}" is not implemented (only "symbol" is supported).`,
    );
  }
  _requireDigitOption('minimumFractionDigits', options.minimumFractionDigits);
  _requireDigitOption('maximumFractionDigits', options.maximumFractionDigits);
  if (
    options.minimumFractionDigits !== undefined &&
    options.maximumFractionDigits !== undefined &&
    options.minimumFractionDigits > options.maximumFractionDigits
  ) {
    throw new RangeError(
      'maximumFractionDigits value is out of range (less than minimumFractionDigits)',
    );
  }
  if (options.style === 'currency') {
    if (options.currency === undefined) {
      throw new TypeError('Currency code is required with currency style');
    }
    if (!/^[a-zA-Z]{3}$/.test(String(options.currency))) {
      throw new RangeError(`Invalid currency code: ${options.currency}`);
    }
    options.currency = String(options.currency).toUpperCase();
  }
}

/**
 * en-US `Intl.NumberFormat`.
 * @constructor
 * @param {string | string[]} [locales]
 * @param {Record<string, any>} [options]
 */
function NumberFormat(locales, options) {
  void locales;
  const o = Object.assign({}, options ?? {});
  _validateNumberOptions(o);
  /** @type {Record<string, any>} */
  this._opts = o;
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
 * en-US `Intl.RelativeTimeFormat`.
 * @constructor
 * @param {string | string[]} [locales]
 * @param {Record<string, any>} [options]
 */
function RelativeTimeFormat(locales, options) {
  void locales;
  const o = options ?? {};
  _rejectUnsupported('RelativeTimeFormat', o, ['localeMatcher']);
  _requireOneOf('RelativeTimeFormat', 'numeric', o.numeric, ['always', 'auto']);
  _requireOneOf('RelativeTimeFormat', 'style', o.style, ['long', 'short', 'narrow']);
  if (o.style === 'short' || o.style === 'narrow') {
    throw new Error(
      `zero test: Intl.RelativeTimeFormat option "style" value "${o.style}" ` +
      'is not implemented (only "long" is supported).',
    );
  }
  /** @type {string} */
  this._numeric = o.numeric === 'auto' ? 'auto' : 'always';
  /** @type {string} */
  this._style = o.style || 'long';
}

/** @type {string[]} */
const RTF_UNITS = ['second', 'minute', 'hour', 'day', 'week', 'month', 'quarter', 'year'];

/**
 * @param {number} value
 * @param {string} unit
 * @returns {string}
 */
RelativeTimeFormat.prototype.format = function format(value, unit) {
  const v = Number(value);
  if (!Number.isFinite(v)) {
    throw new RangeError(
      'Value need to be finite number for Intl.RelativeTimeFormat.prototype.format()',
    );
  }
  const u = _normalizeUnit(unit);
  if (!RTF_UNITS.includes(u)) {
    throw new RangeError(`Invalid unit argument for format() '${unit}'`);
  }
  if (this._numeric === 'auto') return _formatRelativeAuto(v, u);
  return _formatRelativeAlways(v, u);
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
