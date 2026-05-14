import { App, signal } from "zero";
import Home from "./routes/home.ts";

const app = new App();
app.state("count", signal(0));
app.route("/", Home);
app.run("#app");
