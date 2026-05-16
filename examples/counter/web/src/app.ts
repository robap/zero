import { App, signal, effect, html } from "zero";
import Header from "./components/Header.ts";
import { theme } from "./stores/theme.ts";
import Home from "./routes/home.ts";

const app = new App();
app.state("count", signal(0));

// Reflect theme into the document. `null` = follow the system pref
// (drop `data-theme`); a concrete value pins the override.
effect(() => {
  const t = theme.val;
  const root = document.documentElement;
  if (!root) return;
  if (t === null) root.removeAttribute("data-theme");
  else root.setAttribute("data-theme", t);
});

// Layout wraps every route's outlet with the persistent header.
app.layout(({ outlet }) => html`${Header()}<main class="app-main">${outlet}</main>`);
app.route("/", Home);
app.run("#app");
