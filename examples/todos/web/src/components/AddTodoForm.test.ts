import { describe, it, expect, afterEach, beforeEach } from "zero/test";
import { render, find, fire, cleanup } from "zero/test";
import AddTodoForm from "./AddTodoForm.ts";
import { todos } from "../stores/todos.ts";

describe("AddTodoForm", () => {
  beforeEach(() => {
    todos.set({ items: [], filter: "all" });
  });
  afterEach(cleanup);

  it("submitting an empty form is a no-op", () => {
    const el = render(AddTodoForm());
    fire(find(el, "form")!, "submit");
    expect(todos.val.items.length).toBe(0);
  });

  it("submitting with text adds a todo", () => {
    const el = render(AddTodoForm());
    fire(find(el, "input")!, "input", { target: { value: "buy milk" } });
    fire(find(el, "form")!, "submit");
    expect(todos.val.items.length).toBe(1);
    expect(todos.val.items[0].text).toBe("buy milk");
  });
});
