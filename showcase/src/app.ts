import { App, signal, effect } from "zero";
import Home from "./routes/home.ts";
import AvatarRoute from "./routes/avatar.ts";
import BadgeRoute from "./routes/badge.ts";
import ButtonRoute from "./routes/button.ts";
import CardRoute from "./routes/card.ts";
import CheckboxRoute from "./routes/checkbox.ts";
import DialogRoute from "./routes/dialog.ts";
import InputRoute from "./routes/input.ts";
import RadioRoute from "./routes/radio.ts";
import SelectRoute from "./routes/select.ts";
import SpinnerRoute from "./routes/spinner.ts";
import TabsRoute from "./routes/tabs.ts";
import TextAreaRoute from "./routes/textarea.ts";
import ToastRoute from "./routes/toast.ts";
import ToggleRoute from "./routes/toggle.ts";

export type ThemeMode = "auto" | "light" | "dark";

const theme = signal<ThemeMode>("auto");

effect(() => {
  const t = theme.val;
  if (t === "auto") {
    document.documentElement.removeAttribute("data-theme");
  } else {
    document.documentElement.dataset.theme = t;
  }
});

const app = new App();
app.state("theme", theme);
app.route("/", Home);
app.route("/avatar", AvatarRoute);
app.route("/badge", BadgeRoute);
app.route("/button", ButtonRoute);
app.route("/card", CardRoute);
app.route("/checkbox", CheckboxRoute);
app.route("/dialog", DialogRoute);
app.route("/input", InputRoute);
app.route("/radio", RadioRoute);
app.route("/select", SelectRoute);
app.route("/spinner", SpinnerRoute);
app.route("/tabs", TabsRoute);
app.route("/textarea", TextAreaRoute);
app.route("/toast", ToastRoute);
app.route("/toggle", ToggleRoute);
app.run("#app");
