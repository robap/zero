// Cross-coverage for T-rules in routes/.
export function BadRoute() {
  const el = {} as any;
  // T01
  el.removeEventListener("click", () => {});
  // T04 — DOM mutation without ref().el
  const container = {} as any;
  container.appendChild({} as any);
}
