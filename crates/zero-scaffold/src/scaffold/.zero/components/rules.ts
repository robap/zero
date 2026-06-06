/**
 * A validator produced by a rule factory; ignores cross-field values.
 * Single-parameter on purpose: a `Rule` is assignable wherever a
 * field validator is expected.
 */
export type Rule = (value: string) => string | null;

/** Options accepted by every rule factory except `required`. */
export type RuleOptions = {
  /** Replaces the rule's default message. */
  message?: string;
  /**
   * When false, the rule also rejects empty (whitespace-only) values.
   * Default true: empty passes, so optional fields compose.
   */
  allowEmpty?: boolean;
};

/**
 * Resolve a rule factory's final argument: a plain string is shorthand
 * for `{ message }`; `allowEmpty` defaults to true.
 *
 * @param opts The factory's final argument, if any.
 * @returns The resolved message and empty-handling flag.
 * @internal
 */
function resolveOptions(opts?: string | RuleOptions): {
  message?: string;
  allowEmpty: boolean;
} {
  if (typeof opts === "string") return { message: opts, allowEmpty: true };
  return { message: opts?.message, allowEmpty: opts?.allowEmpty ?? true };
}

/**
 * Whether a value is empty after trimming.
 *
 * @param value The value to test.
 * @returns True when the trimmed value is the empty string.
 * @internal
 */
function isEmpty(value: string): boolean {
  return value.trim() === "";
}

/**
 * Require a non-empty (after trimming) value.
 *
 * @param message Replaces the default message.
 * @returns The rule.
 */
export function required(message?: string): Rule {
  return (value) => (isEmpty(value) ? (message ?? "This field is required.") : null);
}

/**
 * Require the trimmed value to parse as a whole number in
 * `[min, max]` inclusive. Accepts an optional leading `+`/`-` and
 * leading zeros; rejects decimals, exponents (`1e3`), and any other
 * non-digit characters. Empty values pass unless `allowEmpty: false`.
 *
 * @param min The lower bound, inclusive.
 * @param max The upper bound, inclusive.
 * @param opts A custom message, or options.
 * @returns The rule.
 */
export function intRange(min: number, max: number, opts?: string | RuleOptions): Rule {
  const { message, allowEmpty } = resolveOptions(opts);
  const fallback = `Must be a whole number between ${min} and ${max}.`;
  return (value) => {
    if (allowEmpty && isEmpty(value)) return null;
    const trimmed = value.trim();
    if (!/^[+-]?\d+$/.test(trimmed)) return message ?? fallback;
    const n = Number(trimmed);
    return min <= n && n <= max ? null : (message ?? fallback);
  };
}

/**
 * Require the trimmed value to look like an email address. The check
 * is pragmatic (`x@y.z` shaped, no whitespace), not RFC 5322 — real
 * verification belongs on the server. Empty values pass unless
 * `allowEmpty: false`.
 *
 * @param opts A custom message, or options.
 * @returns The rule.
 */
export function email(opts?: string | RuleOptions): Rule {
  const { message, allowEmpty } = resolveOptions(opts);
  return (value) => {
    if (allowEmpty && isEmpty(value)) return null;
    return /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(value.trim())
      ? null
      : (message ?? "Enter a valid email address.");
  };
}

/**
 * Require the raw (untrimmed) value to match `re`. The default
 * message — "Invalid format." — names nothing about the pattern, so
 * pass a custom message describing the expected format wherever the
 * rule can actually fire. Empty values pass unless `allowEmpty: false`.
 *
 * @param re The pattern to test. Copied internally with the `g`/`y`
 *   flags stripped so a stateful regex can't alternate results via
 *   `lastIndex`.
 * @param opts A custom message, or options.
 * @returns The rule.
 */
export function pattern(re: RegExp, opts?: string | RuleOptions): Rule {
  const { message, allowEmpty } = resolveOptions(opts);
  const safe = new RegExp(re.source, re.flags.replace(/[gy]/g, ""));
  return (value) => {
    if (allowEmpty && isEmpty(value)) return null;
    return safe.test(value) ? null : (message ?? "Invalid format.");
  };
}

/**
 * Require a trimmed length of at most `n` characters. Empty values
 * pass unless `allowEmpty: false`.
 *
 * @param n The maximum trimmed length, inclusive.
 * @param opts A custom message, or options.
 * @returns The rule.
 */
export function maxLength(n: number, opts?: string | RuleOptions): Rule {
  const { message, allowEmpty } = resolveOptions(opts);
  const fallback = `Must be ${n} character${n === 1 ? "" : "s"} or fewer.`;
  return (value) => {
    if (allowEmpty && isEmpty(value)) return null;
    return value.trim().length <= n ? null : (message ?? fallback);
  };
}

/**
 * Require a trimmed length of at least `n` characters. Empty values
 * pass unless `allowEmpty: false`.
 *
 * @param n The minimum trimmed length, inclusive.
 * @param opts A custom message, or options.
 * @returns The rule.
 */
export function minLength(n: number, opts?: string | RuleOptions): Rule {
  const { message, allowEmpty } = resolveOptions(opts);
  const fallback = `Must be at least ${n} character${n === 1 ? "" : "s"}.`;
  return (value) => {
    if (allowEmpty && isEmpty(value)) return null;
    return value.trim().length >= n ? null : (message ?? fallback);
  };
}
