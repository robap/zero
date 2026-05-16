import { describe, it, expect, afterEach, beforeEach } from "zero/test";
import { render, find, fire, cleanup } from "zero/test";
import TodoItem from "./TodoItem.ts";
import { todos } from "../stores/todos.ts";
import { Keys } from "../state.ts";

describe("TodoItem", () => {
  beforeEach(() => {
    todos.set({ items: [], filter: "all" });
  });
  afterEach(cleanup);

  it("clicking the checkbox toggles done in the store", () => {
    todos.set({
      items: [{ id: "1", text: "a", done: false }],
      filter: "all",
    });
    const el = render(TodoItem({ todo: todos.val.items[0] }), {
      state: { [Keys.Todos]: todos },
    });
    fire(find(el, 'input[type="checkbox"]')!, "change");
    expect(todos.val.items[0].done).toBeTruthy();
  });

  it("clicking the delete button removes the item", () => {
    todos.set({
      items: [{ id: "1", text: "a", done: false }],
      filter: "all",
    });
    const el = render(TodoItem({ todo: todos.val.items[0] }), {
      state: { [Keys.Todos]: todos },
    });
    // The shipped Button component emits `class="button button-ghost button-sm"`.
    fire(find(el, "button.button-ghost")!, "click");
    expect(todos.val.items.length).toBe(0);
  });
});
