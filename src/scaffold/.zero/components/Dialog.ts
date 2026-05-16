import { html, effect } from "zero";
import type { Signal, TemplateResult } from "zero";

export type DialogSize = "sm" | "md" | "lg";

export type DialogProps = {
  open: Signal<boolean>;
  size?: DialogSize;
  title?: string;
  children?: TemplateResult | string;
  onClose?: () => void;
};

/**
 * Dialog — modal overlay with a backdrop. Closes on backdrop click or
 * Escape; closes-by-calling `open.set(false)`. No focus trap (v1).
 *
 * @param props
 * @returns
 */
export default function Dialog(props: DialogProps): TemplateResult {
  const size: DialogSize = props.size ?? "md";
  const dialogCls = `dialog dialog-${size}`;
  const close = () => {
    props.open.set(false);
    props.onClose?.();
  };

  effect(() => {
    if (!props.open.val) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") close();
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  });

  const header: TemplateResult | null = props.title
    ? html`<h2 class="dialog-title">${props.title}</h2>`
    : null;

  const body = (): TemplateResult | null => {
    if (!props.open.val) return null;
    return html`<div class="dialog-backdrop dialog-open" @click=${close}><div class=${dialogCls} role="dialog" aria-modal="true" @click.stop=${() => {}}>${header}<div class="dialog-body">${props.children ?? ""}</div></div></div>`;
  };

  return html`${body}`;
}
