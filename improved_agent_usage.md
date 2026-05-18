part of what I am trying with this demo is to see how well agents can use my zero framework.
  What would you say is the reason you did not use the provided css?

● Honest answer, because you asked: I never actually read the framework. I read about it.

  What that means concretely:

  1. I treated AGENTS.md as a survey, not a spec. I read it once at the top of the session, noted
  "layout primitives exist," "tokens exist," "typography utilities exist," and moved on. I never
  opened _tokens.scss to see the full token table. I never opened _typography.scss to see the dozen
   utilities. The summary in AGENTS.md gave me enough vocabulary to talk about the system without
  ever consulting the API surface. When you called me out on pad-*, then layout primitives, then
  typography/tokens — each time the same fix: open the partial, read what's there, use it. I should
   have done that on day one.
  2. My prior CSS habits overrode my fresh-read knowledge. font-weight: 600, border-radius: 999px,
  font-size: 0.75rem are muscle memory from generic CSS work. When I needed a small bold uppercase
  chip, I autocompleted those properties instead of asking "is there a named primitive for exactly
  this?" My background knowledge silently won over the framework's vocabulary every time the
  framework hadn't fully replaced what I already knew. The token system is light enough on
  enforcement that nothing in my feedback loop pushed back.
  3. No feedback signal flagged it. TDD caught my TypeScript shortcuts because tests fail loudly.
  CSS shortcuts produced a clean zero build and visually-plausible output (in my head — I never
  opened a browser either). The framework currently has no lint step that says "you wrote
  font-weight: 600; did you mean var(--weight-semi)?" So my CSS rode on whatever I happened to
  type. Once it built, it was "done."
  4. I confused "uses tokens" with "follows the system." I felt good about var(--space-md),
  var(--color-surface), var(--border-thin) — the obvious tokens. Radius pill, weight tokens,
  tracking tokens, the typography utilities, the pad-* family — those are second-tier knowledge
  that lives inside the partials. I stopped at the surface and didn't go deeper until you pushed.

  Things that would have prevented this for me, ranked by likely effectiveness:

  - A lint pass. zero lint (or build warnings) that flags raw font-weights, font-sizes, tracking
  values, radius values, and hex/rgb colors in user SCSS, suggesting the token. This is the
  strongest signal because it doesn't require me to remember anything — the framework tells me.
  - AGENTS.md should embed the actual token table and utility-class list inline, not just describe
  the partials. Right now reading AGENTS.md gives me names; reading the partials gives me values.
  An agent that only reads AGENTS.md cannot tell that --radius-pill exists.
  - A negative-examples section in AGENTS.md: "Don't write font-weight: 600 — use
  var(--weight-semi). Don't write border-radius: 999px — use var(--radius-pill). Don't write
  font-size: 0.875rem — use var(--font-size-sm) or the text-small utility." A handful of these
  covers ~90% of what I got wrong, and they're memorable because they're concrete.
  - Make the typography utilities the default route. Phrase the guidance as "use a text-* utility
  for any visible text element; fall back to custom CSS only for non-text concerns (sizing,
  position, color of non-text elements)." That inverts my default behavior.

  The meta-point: framework adoption fails the same way for agents as for humans — you skim docs
  and lean on what you already know. The difference is that agents skim more aggressively and have
  stronger priors from generic web work. If you want agents to use the framework's vocabulary, the
  framework needs feedback loops that make non-framework code visibly wrong, not just suboptimal.
