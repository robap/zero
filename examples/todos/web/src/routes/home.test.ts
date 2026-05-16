import { describe, it, expect, afterEach, beforeEach } from "zero/test";
import { render, findAll, find, fire, cleanup } from "zero/test";
import { signal } from "zero";
import Home from "./home.ts";
import { todos } from "../stores/todos.ts";
import { Keys } from "../state.ts";

describe("Home (todos)", () => {
  beforeEach(() => {
    todos.set({ items: [], filter: "all" });
  });
  afterEach(cleanup);

  it("renders the seeded list", () => {
    todos.set({
      items: [
        { id: "1", text: "buy milk", done: false },
        { id: "2", text: "walk dog", done: true },
      ],
      filter: "all",
    });
    const el = render(Home(), { state: { [Keys.Todos]: todos } });
    const items = findAll(el, "li");
    expect(items.length).toBe(2);
  });

  it("adding via the form appends a new item to the rendered list", () => {
    const el = render(Home(), { state: { [Keys.Todos]: todos } });
    fire(find(el, "input")!, "input", { target: { value: "new item" } });
    fire(find(el, "form")!, "submit");
    const items = findAll(el, "li");
    expect(items.length).toBe(1);
  });
});
