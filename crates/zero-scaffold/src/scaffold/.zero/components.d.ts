// Declared so editors can resolve the bare specifier "zero/components"
// against the source under .zero/components/. Populated by per-component
// steps; entries inside the module block are kept alphabetical.
declare module "zero/components" {
  import type { Signal, TemplateResult } from "zero";

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
  export type ButtonProps = {
    variant?: ButtonVariant;
    size?: ButtonSize;
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
  };
  export function Checkbox(props: CheckboxProps): TemplateResult;

  export type DialogSize = "sm" | "md" | "lg";
  export type DialogProps = {
    open: Signal<boolean>;
    size?: DialogSize;
    title?: string;
    children?: TemplateResult | string;
    onClose?: () => void;
  };
  export function Dialog(props: DialogProps): TemplateResult;

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
  };
  export function Input(props: InputProps): TemplateResult;

  export type RadioProps = {
    selected: Signal<string>;
    name: string;
    value: string;
    label?: string;
    disabled?: boolean;
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
  };
  export function Toggle(props: ToggleProps): TemplateResult;
}
