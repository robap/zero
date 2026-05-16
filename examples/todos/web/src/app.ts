import { App, effect, html } from "zero";
import { Keys } from "./state.ts";
import { todos, persistTodos } from "./stores/todos.ts";
import { theme } from "./stores/theme.ts";
import Header from "./components/Header.ts";
import Home from "./routes/home.ts";

const app = new App();
app.state(Keys.Todos, todos);
app.state(Keys.Theme, theme);

// Persist on every change. Reading `todos.val` once registers the dep;
// `persistTodos()` reads the current value from the store.
effect(() => {
  // Touch all fields we want to watch.
  todos.val;
  persistTodos();
});

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
