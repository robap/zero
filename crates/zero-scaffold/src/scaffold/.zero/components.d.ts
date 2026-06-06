// Declared so editors can resolve the bare specifier "zero/components"
// against the source under .zero/components/. Populated by per-component
// steps; entries inside the module block are kept alphabetical.
declare module "zero/components" {
  import type { Signal, Computed, TemplateResult } from "zero";

  /**
   * Native attributes accepted by every form control's `attrs` prop.
   * Applied additively after mount: attributes the component renders
   * itself win and the colliding key is skipped. `true` sets an empty
   * attribute, `false` skips the key, numbers are stringified.
   */
  export type NativeAttrs = Record<string, string | number | boolean>;

  export type AvatarSize = "sm" | "md" | "lg" | "xl";
  export type AvatarProps = {
    src?: string;
    alt: string;
    initials?: string;
    size?: AvatarSize;
  };
  export function Avatar(props: AvatarProps): TemplateResult;

  export type BadgeVariant = "default" | "primary" | "success" | "warning" | "danger";
  export type BadgeSize = "sm" | "md";
  export type BadgeProps = {
    variant?: BadgeVariant;
    size?: BadgeSize;
    children?: TemplateResult | string;
  };
  export function Badge(props?: BadgeProps): TemplateResult;

  export type ButtonVariant = "primary" | "secondary" | "ghost" | "danger";
  export type ButtonSize = "sm" | "md" | "lg";
  export type ButtonType = "button" | "submit" | "reset";
  export type ButtonProps = {
    variant?: ButtonVariant;
    size?: ButtonSize;
    type?: ButtonType;
    form?: string;
    name?: string;
    value?: string;
    disabled?: boolean;
    loading?: boolean;
    onClick?: (event: Event) => void;
    children?: TemplateResult | string;
  };
  export function Button(props?: ButtonProps): TemplateResult;

  export type CardVariant = "surface" | "outlined";
  export type CardProps = {
    variant?: CardVariant;
    title?: string;
    children?: TemplateResult | string;
  };
  export function Card(props?: CardProps): TemplateResult;

  export type CheckboxProps = {
    checked: Signal<boolean>;
    label?: string;
    disabled?: boolean;
    debounceMs?: number;
    error?: Signal<string | null>;
    /** Focus the inner checkbox `<input>` after mount. */
    autofocus?: boolean;
    /** Additive-only native attributes for the inner checkbox `<input>`. */
    attrs?: NativeAttrs;
  };
  export function Checkbox(props: CheckboxProps): TemplateResult;

  export type ComboboxSize = "sm" | "md" | "lg";
  export type ComboboxOption = { value: string; label: string };
  export type ComboboxProps = {
    value: Signal<string>;
    loadOptions: (query: string) => Promise<ComboboxOption[]>;
    initialLabel?: string;
    size?: ComboboxSize;
    placeholder?: string;
    label?: string;
    disabled?: Signal<boolean> | Computed<boolean> | boolean;
    debounceMs?: number;
    minQueryLength?: number;
    noResultsLabel?: string;
    loadingLabel?: string;
    onChange?: (value: string, option: ComboboxOption) => void;
    /**
     * Allow free text: blur/outside-click/Enter-without-ghost commits the
     * trimmed visible text to `value`; a case-insensitive whole-label match
     * resolves to that option; otherwise `onChange` gets a synthesized
     * `{ value: text, label: text }`. Defaults to false (strict revert).
     */
    allowCustom?: boolean;
    error?: Signal<string | null>;
    /** Focus the inner typeahead `<input>` after mount. */
    autofocus?: boolean;
    /** Additive-only native attributes for the inner typeahead `<input>`. */
    attrs?: NativeAttrs;
  };
  export function Combobox(props: ComboboxProps): TemplateResult;

  /**
   * Per-field validator: return an error message or `null` when valid.
   * Receives the field's current value and a snapshot of all values.
   */
  export type Validator<K extends string = string> = (
    value: string,
    values: Record<K, string>,
  ) => string | null;
  export type FieldConfig<K extends string> = {
    initial: string;
    /** One validator or an array run in order; first non-null message wins. */
    validate?: Validator<K> | Validator<K>[];
  };
  export type FormField = {
    value: Signal<string>;
    error: Signal<string | null>;
    touched: Signal<boolean>;
  };
  export type FormConfig<K extends string> = {
    fields: Record<K, FieldConfig<K>>;
    validate?: (values: Record<K, string>) => Partial<Record<K, string>>;
  };
  export type SubmitAction<K extends string> = (
    values: Record<K, string>,
  ) => void | Promise<void>;
  export type Form<K extends string> = {
    fields: Record<K, FormField>;
    isValid: Computed<boolean>;
    error: Signal<string | null>;
    values(): Record<K, string>;
    reset(): void;
    setErrors(errors: Partial<Record<K, string>>): void;
    submit(action: SubmitAction<K>): (e: Event) => Promise<void>;
  };
  export function createForm<K extends string>(config: FormConfig<K>): Form<K>;

  /** A validator produced by a rule factory; ignores cross-field values. */
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
  export function required(message?: string): Rule;
  export function minLength(n: number, opts?: string | RuleOptions): Rule;
  export function maxLength(n: number, opts?: string | RuleOptions): Rule;
  export function intRange(
    min: number,
    max: number,
    opts?: string | RuleOptions,
  ): Rule;
  export function pattern(re: RegExp, opts?: string | RuleOptions): Rule;
  export function email(opts?: string | RuleOptions): Rule;

  export type DialogSize = "sm" | "md" | "lg";
  export type DialogProps = {
    open: Signal<boolean>;
    size?: DialogSize;
    title?: string;
    children?: TemplateResult | string;
    onClose?: () => void;
  };
  export function Dialog(props: DialogProps): TemplateResult;

  export type DrawerSide = "left" | "right" | "top" | "bottom";
  export type DrawerMode = "overlay" | "push";
  export type DrawerSize = "sm" | "md" | "lg";
  export type DrawerSlot =
    | TemplateResult
    | string
    | null
    | undefined
    | (() => TemplateResult | string | null);
  export type DrawerProps = {
    open: Signal<boolean>;
    side: DrawerSide;
    mode?: DrawerMode;
    size?: DrawerSize;
    title?: DrawerSlot;
    body?: DrawerSlot;
    controls?: DrawerSlot;
  };
  export function Drawer(props: DrawerProps): TemplateResult;

  export type InputType =
    | "text"
    | "email"
    | "password"
    | "number"
    | "search"
    | "url"
    | "tel";
  export type InputSize = "sm" | "md" | "lg";
  export type InputProps = {
    value: Signal<string>;
    type?: InputType;
    size?: InputSize;
    placeholder?: string;
    disabled?: boolean;
    label?: string;
    debounceMs?: number;
    onChange?: (value: string) => void;
    error?: Signal<string | null>;
    /** Focus the underlying `<input>` after mount. */
    autofocus?: boolean;
    /** Additive-only native attributes for the underlying `<input>`. */
    attrs?: NativeAttrs;
  };
  export function Input(props: InputProps): TemplateResult;

  export type PaginationSize = "sm" | "md" | "lg";
  export type PaginationProps = {
    page: Signal<number>;
    totalPages: Signal<number> | Computed<number> | number;
    size?: PaginationSize;
    siblingCount?: number;
    boundaryCount?: number;
    disabled?: Signal<boolean> | Computed<boolean> | boolean;
    onChange?: (page: number) => void;
    prevLabel?: string;
    nextLabel?: string;
    summary?: (page: number, totalPages: number) => TemplateResult | string;
  };
  export function Pagination(props: PaginationProps): TemplateResult;

  export type RadioProps = {
    selected: Signal<string>;
    name: string;
    value: string;
    label?: string;
    disabled?: boolean;
    debounceMs?: number;
    error?: Signal<string | null>;
    /** Focus the inner radio `<input>` after mount. */
    autofocus?: boolean;
    /**
     * Additive-only native attributes for the inner radio `<input>`.
     * `name`/`value` are first-class props and always win here.
     */
    attrs?: NativeAttrs;
  };
  export function Radio(props: RadioProps): TemplateResult;

  export type SelectSize = "sm" | "md" | "lg";
  export type SelectOption = { value: string; label: string };
  export type SelectProps = {
    value: Signal<string>;
    options: SelectOption[];
    size?: SelectSize;
    disabled?: boolean;
    label?: string;
    debounceMs?: number;
    onChange?: (value: string) => void;
    error?: Signal<string | null>;
    /** Focus the underlying `<select>` after mount. */
    autofocus?: boolean;
    /** Additive-only native attributes for the underlying `<select>`. */
    attrs?: NativeAttrs;
  };
  export function Select(props: SelectProps): TemplateResult;

  export type SpinnerVariant = "primary" | "muted";
  export type SpinnerSize = "sm" | "md" | "lg";
  export type SpinnerProps = {
    variant?: SpinnerVariant;
    size?: SpinnerSize;
    label?: string;
  };
  export function Spinner(props?: SpinnerProps): TemplateResult;

  export type TabsTab = { id: string; label: string };
  export type TabsProps = {
    active: Signal<string>;
    tabs: TabsTab[];
    panels: Record<string, TemplateResult>;
  };
  export function Tabs(props: TabsProps): TemplateResult;

  export type TableDensity = "compact" | "cozy";
  export type TableColumn<T> = {
    key: keyof T & string;
    label: string;
    align?: "start" | "end" | "center";
    width?: string;
    render?: (row: T, i: number) => TemplateResult | string | number;
  };
  export type TableProps<T> = {
    columns: TableColumn<T>[];
    rows: Signal<T[]>;
    rowKey: (row: T, i: number) => string | number;
    onRowClick?: (row: T, i: number) => void;
    density?: TableDensity;
    maxHeight?: string;
    empty?: TemplateResult;
    loading?: Signal<boolean>;
  };
  export function Table<T>(props: TableProps<T>): TemplateResult;

  export type TextAreaProps = {
    value: Signal<string>;
    rows?: number;
    placeholder?: string;
    disabled?: boolean;
    label?: string;
    debounceMs?: number;
    error?: Signal<string | null>;
    /** Focus the underlying `<textarea>` after mount. */
    autofocus?: boolean;
    /** Additive-only native attributes for the underlying `<textarea>`. */
    attrs?: NativeAttrs;
  };
  export function TextArea(props: TextAreaProps): TemplateResult;

  export type ToastVariant = "info" | "success" | "warning" | "danger";
  export type ToastProps = {
    open: Signal<boolean>;
    variant?: ToastVariant;
    message: string;
    duration?: number;
    onDismiss?: () => void;
  };
  export function Toast(props: ToastProps): TemplateResult;

  export type ToggleProps = {
    checked: Signal<boolean>;
    label?: string;
    disabled?: boolean;
    debounceMs?: number;
    error?: Signal<string | null>;
    /** Focus the inner switch `<input>` after mount. */
    autofocus?: boolean;
    /** Additive-only native attributes for the inner switch `<input>`. */
    attrs?: NativeAttrs;
  };
  export function Toggle(props: ToggleProps): TemplateResult;
}
