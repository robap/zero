import { html } from "zero";
import type { Signal, TemplateResult } from "zero";
import { read, type Reactive } from "./_internal.ts";

export type PaginationSize = "sm" | "md" | "lg";

export type PaginationProps = {
  page: Signal<number>;
  totalPages: Reactive<number> | number;
  size?: PaginationSize;
  siblingCount?: number;
  boundaryCount?: number;
  disabled?: Reactive<boolean> | boolean;
  onChange?: (page: number) => void;
  prevLabel?: string;
  nextLabel?: string;
  summary?: (page: number, totalPages: number) => TemplateResult | string;
};

/**
 * Inclusive integer range. Returns an empty array when `end < start`.
 *
 * @param start
 * @param end
 * @returns
 * @internal
 */
function range(start: number, end: number): number[] {
  if (end < start) return [];
  return Array.from({ length: end - start + 1 }, (_, i) => start + i);
}

/**
 * Build the visible page item array. The algorithm follows the
 * MUI-style "shift sibling window when it crowds a boundary" strategy:
 * when the gap between the sibling window and a boundary is exactly one
 * page, that page is rendered instead of an ellipsis.
 *
 * @param page Current (clamped) page.
 * @param total Total number of pages, >= 1.
 * @param sibling Number of pages on each side of current.
 * @param boundary Number of pages at each end.
 * @returns
 * @internal
 */
function pageItems(
  page: number,
  total: number,
  sibling: number,
  boundary: number,
): (number | "...")[] {
  const startPages = range(1, Math.min(boundary, total));
  const endPages = range(Math.max(total - boundary + 1, boundary + 1), total);
  const siblingsStart = Math.max(
    Math.min(page - sibling, total - boundary - sibling * 2 - 1),
    boundary + 2,
  );
  const siblingsEnd = Math.min(
    Math.max(page + sibling, boundary + sibling * 2 + 2),
    endPages.length > 0 ? endPages[0]! - 2 : total - 1,
  );
  const items: (number | "...")[] = [...startPages];
  if (siblingsStart > boundary + 2) items.push("...");
  else if (boundary + 1 < total - boundary) items.push(boundary + 1);
  items.push(...range(siblingsStart, siblingsEnd));
  if (siblingsEnd < total - boundary - 1) items.push("...");
  else if (total - boundary > boundary) items.push(total - boundary);
  items.push(...endPages);
  return items;
}

/**
 * Pagination — numbered pager with prev/next buttons and ellipsis.
 * Controlled by `page: Signal<number>`. `totalPages` and `disabled`
 * accept a `Signal`, a `Computed`, or a plain value so async parents
 * can update them without remount. Pages are 1-indexed. Visual
 * treatment composes `.button` + `.button-{ghost,primary}` +
 * `.button-${size}` so this component carries no duplicated button
 * CSS.
 *
 * @param props
 * @returns
 */
export default function Pagination(props: PaginationProps): TemplateResult {
  const size: PaginationSize = props.size ?? "md";
  const sibling = props.siblingCount ?? 1;
  const boundary = props.boundaryCount ?? 1;
  const prevLabel = props.prevLabel ?? "Previous";
  const nextLabel = props.nextLabel ?? "Next";
  const btnBase = `button button-${size} pagination-btn`;
  const ghostCls = `${btnBase} button-ghost`;
  const activeCls = `${btnBase} button-primary pagination-active`;

  const resolvedTotal = (): number => Math.max(1, read(props.totalPages));
  const clampedPage = (): number => {
    const t = resolvedTotal();
    const p = props.page.val;
    return p < 1 ? 1 : p > t ? t : p;
  };
  const isDisabled = (): boolean =>
    read(props.disabled) === true || resolvedTotal() <= 1;

  const go = (n: number): void => {
    if (isDisabled()) return;
    const t = resolvedTotal();
    const target = n < 1 ? 1 : n > t ? t : n;
    if (target === clampedPage()) return;
    props.page.set(target);
    props.onChange?.(target);
  };

  const pageBtn = (n: number, active: boolean, dis: boolean): TemplateResult => html`
    <li>
      <button
        class=${active ? activeCls : ghostCls}
        aria-label=${`Page ${n}`}
        aria-current=${active ? "page" : null}
        disabled=${dis}
        @click=${() => go(n)}
      >${n}</button>
    </li>
  `;

  const ellipsis = (): TemplateResult => html`
    <li><span class="pagination-ellipsis text-muted" aria-hidden="true">…</span></li>
  `;

  const prevBtn = (cp: number, dis: boolean): TemplateResult => html`
    <li>
      <button
        class=${`${ghostCls} pagination-prev`}
        aria-label=${prevLabel}
        disabled=${dis || cp <= 1}
        @click=${() => go(cp - 1)}
      >‹</button>
    </li>
  `;

  const nextBtn = (cp: number, t: number, dis: boolean): TemplateResult => html`
    <li>
      <button
        class=${`${ghostCls} pagination-next`}
        aria-label=${nextLabel}
        disabled=${dis || cp >= t}
        @click=${() => go(cp + 1)}
      >›</button>
    </li>
  `;

  const listBlock = (): TemplateResult[] => {
    const cp = clampedPage();
    const t = resolvedTotal();
    const dis = isDisabled();
    const middle = pageItems(cp, t, sibling, boundary).map((it) =>
      it === "..." ? ellipsis() : pageBtn(it, it === cp, dis),
    );
    return [prevBtn(cp, dis), ...middle, nextBtn(cp, t, dis)];
  };

  const navCls = (): string =>
    `pagination pagination-${size} stack gap-sm${isDisabled() ? " pagination-disabled" : ""}`;

  const summaryBlock = props.summary
    ? (): TemplateResult =>
        html`<div class="pagination-summary text-small">${props.summary!(clampedPage(), resolvedTotal())}</div>`
    : null;

  return html`
    <nav class=${navCls} role="navigation" aria-label="Pagination">
      ${summaryBlock}
      <ul class="pagination-list cluster gap-xs">${listBlock}</ul>
    </nav>
  `;
}
