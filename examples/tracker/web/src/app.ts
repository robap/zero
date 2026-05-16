import { App, effect, navigate, html } from "zero";
import { Keys } from "./state.ts";
import { auth } from "./stores/auth.ts";
import { theme } from "./stores/theme.ts";
import { issues } from "./stores/issues.ts";
import { api } from "./lib/api.ts";
import { requireAuth } from "./lib/guards.ts";
import Header from "./components/Header.ts";
import Home from "./routes/home.ts";
import Login from "./routes/login.ts";
import IssuesIndex, {
  load as loadIssuesIndex,
  meta as issuesIndexMeta,
} from "./routes/issues/index.ts";
import IssuePage, {
  load as loadIssue,
  meta as issueMeta,
} from "./routes/issues/issue.ts";

const app = new App();
app.state(Keys.Auth, auth);
app.state(Keys.Theme, theme);
app.state(Keys.Issues, issues);

// Reflect theme into the document. `null` = follow the system pref
// (drop `data-theme`); a concrete value pins the override.
effect(() => {
  const t = theme.val;
  const root = document.documentElement;
  if (!root) return;
  if (t === null) root.removeAttribute("data-theme");
  else root.setAttribute("data-theme", t);
});

// Root layout: header + main outlet. The global header stays mounted
// across login → dashboard transitions.
app.layout(({ outlet }) => html`${Header()}<main class="app-main">${outlet}</main>`);

app.route("/", Home);
app.route("/login", Login);
app.route("/issues", IssuesIndex, {
  load: loadIssuesIndex,
  meta: issuesIndexMeta,
  guard: requireAuth,
});
app.route("/issues/:id", IssuePage, {
  load: loadIssue,
  meta: issueMeta,
  guard: requireAuth,
});

// HTTP middleware: 401 redirects to /login. Registered here (the
// composition root) rather than inside a store so the policy is
// visible at the place that owns app-wide behavior.
api.use(async (req, next) => {
  const res = await next(req);
  if (res.status === 401) navigate("/login");
  return res;
});

app.run("#app");

// Post-login redirect. With `app.run` already invoked, `navigate` has a
// current app to dispatch through. The effect re-runs whenever
// `auth.val.status` changes; the path check confines redirects to the
// `/login` entry route so we don't fight subsequent in-app navigation.
effect(() => {
  if (auth.val.status === "loggedIn" && window.location.pathname === "/login") {
    navigate("/issues");
  }
});
