import { html, effect } from "zero";
import type { Signal, TemplateResult } from "zero";

export type ToastVariant = "info" | "success" | "warning" | "danger";

export type ToastProps = {
  open: Signal<boolean>;
  variant?: ToastVariant;
  message: string;
  duration?: number;
  onDismiss?: () => void;
};

/**
 * Toast — fixed-position transient message. Auto-dismisses after
 * `duration` ms if provided. Single-toast UI; no queue.
 *
 * @param props
 * @returns
 */
export default function Toast(props: ToastProps): TemplateResult {
  const variant: ToastVariant = props.variant ?? "info";
  const cls = `toast toast-${variant}`;

  if (props.duration != null) {
    effect(() => {
      if (!props.open.val) return;
      const timer = setTimeout(() => {
        props.open.set(false);
        props.onDismiss?.();
      }, props.duration);
      return () => clearTimeout(timer);
    });
  }

  const body = (): TemplateResult | null => {
    if (!props.open.val) return null;
    return html`<div class=${cls} role="status" aria-live="polite">${props.message}</div>`;
  };

  return html`${body}`;
}
