import { computed, signal } from "zero";
import type { Computed, Signal } from "zero";
import { HttpError } from "zero/http";

/**
 * Form-level message shown when a submit fails for any reason other
 * than a 400/409 response carrying a non-empty `{ errors }` body.
 */
const GENERIC_SUBMIT_ERROR = "Could not save. Try again.";

/**
 * Declaration for a single form field.
 *
 * @template K Union of the form's field names.
 */
export type FieldConfig<K extends string> = {
  /** Initial value; also the value restored by `Form.reset()`. */
  initial: string;
  /**
   * Per-field validator; return an error message or `null` when valid.
   * Receives the field's current value and a snapshot of all values for
   * rules that need context.
   */
  validate?: (value: string, values: Record<K, string>) => string | null;
};

/**
 * Reactive state for a single declared field.
 */
export type FormField = {
  /**
   * The field's value signal — bind directly to a control's `value`
   * prop. Writing marks the field touched.
   */
  value: Signal<string>;
  /**
   * The field's error signal — bind directly to a control's `error`
   * prop. Populated by submit/`setErrors`, cleared live as the user
   * fixes an errored field.
   */
  error: Signal<string | null>;
  /** False until the user first edits the field; reset by `reset()`. */
  touched: Signal<boolean>;
};

/**
 * Configuration accepted by {@link createForm}.
 *
 * @template K Union of the form's field names.
 */
export type FormConfig<K extends string> = {
  /** Field declarations, keyed by field name. */
  fields: Record<K, FieldConfig<K>>;
  /**
   * Cross-field validator; runs after per-field validators and fills
   * only keys that don't already carry a per-field error.
   */
  validate?: (values: Record<K, string>) => Partial<Record<K, string>>;
};

/**
 * The action invoked by `Form.submit()` once client-side validation
 * passes. Builds the typed request body and performs the request and
 * success handling.
 *
 * @template K Union of the form's field names.
 */
export type SubmitAction<K extends string> = (
  values: Record<K, string>,
) => void | Promise<void>;

/**
 * The reactive form object returned by {@link createForm}.
 *
 * @template K Union of the form's field names.
 */
export type Form<K extends string> = {
  /** Per-field reactive state, keyed by field name. */
  fields: Record<K, FormField>;
  /**
   * Live computed: `true` iff running all validators over the current
   * values yields no errors. Drives a disabled submit button without
   * populating any field's error signal.
   */
  isValid: Computed<boolean>;
  /** Form-level error message (server/global failures). */
  error: Signal<string | null>;
  /** Snapshot of current values, keyed by field name. */
  values(): Record<K, string>;
  /**
   * Restore every field to its initial value and clear all field
   * errors, all touched flags, and the form-level error.
   */
  reset(): void;
  /**
   * Apply server-provided field errors: named fields get their message,
   * fields not present are cleared.
   */
  setErrors(errors: Partial<Record<K, string>>): void;
  /**
   * Wrap a submit action into an async `@submit` event handler that
   * validates, gates, and maps server validation errors onto fields.
   */
  submit(action: SubmitAction<K>): (e: Event) => Promise<void>;
};

/**
 * Create a reactive form: typed field/error/touched signals, validator
 * functions, a live `isValid`, and a `submit()` wrapper.
 *
 * @template K Union of the form's field names, inferred from
 *   `config.fields`' keys.
 * @param config Field declarations plus an optional cross-field
 *   validator.
 * @returns The reactive {@link Form} object.
 */
export function createForm<K extends string>(config: FormConfig<K>): Form<K> {
  const keys = Object.keys(config.fields) as K[];
  const inner = {} as Record<K, Signal<string>>;
  const fields = {} as Record<K, FormField>;
  const formError = signal<string | null>(null);

  /**
   * Snapshot the current field values. Reads go through the inner
   * signals so callers inside a `computed` track every field.
   *
   * @returns The current values, keyed by field name.
   */
  const values = (): Record<K, string> => {
    const out = {} as Record<K, string>;
    for (const k of keys) out[k] = inner[k].val;
    return out;
  };

  /**
   * Run every per-field validator, then merge the cross-field
   * validator's result for keys that don't already carry an error.
   *
   * @returns Error messages keyed by field name; valid fields absent.
   */
  const runValidators = (): Partial<Record<K, string>> => {
    const vals = values();
    const errors: Partial<Record<K, string>> = {};
    for (const k of keys) {
      const msg = config.fields[k].validate?.(vals[k], vals);
      if (msg != null) errors[k] = msg;
    }
    if (config.validate) {
      const cross = config.validate(vals);
      for (const k of keys) {
        const msg = cross[k];
        if (msg != null && errors[k] == null) errors[k] = msg;
      }
    }
    return errors;
  };

  for (const k of keys) fields[k] = makeField(config, k, inner, runValidators);

  const isValid = computed(() => {
    const errors = runValidators();
    return keys.every((k) => errors[k] == null);
  });

  /**
   * Restore initials and clear all field errors, touched flags, and the
   * form-level error. Writes the inner signals directly so reset does
   * not mark fields touched.
   */
  const reset = (): void => {
    for (const k of keys) {
      inner[k].set(config.fields[k].initial);
      fields[k].error.set(null);
      fields[k].touched.set(false);
    }
    formError.set(null);
  };

  /**
   * Apply errors to field error signals: named fields get their
   * message, declared fields not present are cleared.
   *
   * @param errors Error messages keyed by field name.
   */
  const setErrors = (errors: Partial<Record<K, string>>): void => {
    for (const k of keys) fields[k].error.set(errors[k] ?? null);
  };

  /**
   * Wrap `action` into an async `@submit` handler: prevent default,
   * mark every field touched, validate and gate, then run the action
   * and map any thrown error onto field/form error signals. Never
   * rethrows.
   *
   * @param action The validated-submit action.
   * @returns An async `@submit` event handler.
   */
  const submit = (action: SubmitAction<K>): ((e: Event) => Promise<void>) => {
    return async (e: Event): Promise<void> => {
      e.preventDefault();
      for (const k of keys) fields[k].touched.set(true);
      const errors = runValidators();
      setErrors(errors);
      formError.set(null);
      if (keys.some((k) => errors[k] != null)) return;
      try {
        await action(values());
      } catch (err) {
        applyServerError(err, keys, fields, formError);
      }
    };
  };

  return { fields, isValid, error: formError, values, reset, setErrors, submit };
}

/**
 * Map an error thrown by a submit action onto the form. A 400/409
 * `HttpError` carrying a non-empty `{ errors }` object sets matching
 * fields' error signals; messages under unmatched keys are joined with
 * a single space (in `Object.entries` order) into the form-level error,
 * never silently dropped. Anything else — missing/empty `errors`, other
 * statuses, non-`HttpError` throws — sets the generic form-level
 * message.
 *
 * @template K Union of the form's field names.
 * @param err The thrown value.
 * @param keys The form's declared field names.
 * @param fields The form's per-field reactive state.
 * @param formError The form-level error signal.
 * @internal
 */
function applyServerError<K extends string>(
  err: unknown,
  keys: K[],
  fields: Record<K, FormField>,
  formError: Signal<string | null>,
): void {
  if (
    err instanceof HttpError &&
    (err.status === 400 || err.status === 409)
  ) {
    const body = err.body as { errors?: unknown } | null | undefined;
    const errors = body != null && typeof body === "object" ? body.errors : undefined;
    if (
      errors != null &&
      typeof errors === "object" &&
      !Array.isArray(errors) &&
      Object.keys(errors).length > 0
    ) {
      const declared = new Set<string>(keys);
      const unmatched: string[] = [];
      for (const [key, msg] of Object.entries(errors as Record<string, unknown>)) {
        if (declared.has(key)) fields[key as K].error.set(String(msg));
        else unmatched.push(String(msg));
      }
      if (unmatched.length > 0) formError.set(unmatched.join(" "));
      return;
    }
  }
  formError.set(GENERIC_SUBMIT_ERROR);
}

/**
 * Build one field's reactive state. The `value` signal is a façade over
 * an inner `signal(initial)`: writes mark the field touched and, iff
 * the field currently shows an error, re-validate just that field so
 * the message clears (or switches) live as the user types — but never
 * appears before submit/`setErrors`.
 *
 * @template K Union of the form's field names.
 * @param config The form configuration.
 * @param key The field's name.
 * @param inner Map of inner value signals; this field's entry is set here.
 * @param runValidators The form's validator runner.
 * @returns The field's reactive state.
 * @internal
 */
function makeField<K extends string>(
  config: FormConfig<K>,
  key: K,
  inner: Record<K, Signal<string>>,
  runValidators: () => Partial<Record<K, string>>,
): FormField {
  const value = signal(config.fields[key].initial);
  inner[key] = value;
  const error = signal<string | null>(null);
  const touched = signal(false);
  const afterWrite = (): void => {
    touched.set(true);
    if (error.val != null) error.set(runValidators()[key] ?? null);
  };
  const facade: Signal<string> = {
    get val(): string {
      return value.val;
    },
    set(v: string): void {
      value.set(v);
      afterWrite();
    },
    update(fn: (current: string) => string): void {
      value.update(fn);
      afterWrite();
    },
  };
  return { value: facade, error, touched };
}
