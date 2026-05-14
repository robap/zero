import { App } from "zero";
import Home from "./routes/home.js";

const app = new App();
app.route("/", Home);
app.run("#app");
