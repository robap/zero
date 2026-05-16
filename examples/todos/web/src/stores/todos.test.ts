import { describe, it, expect, beforeEach } from "zero/test";
import { todos, addTodo, toggleTodo, deleteTodo, editTodo, setFilter } from "./todos.ts";

describe("todos store", () => {
  beforeEach(() => {
    // Reset to a known empty state. Mutators write through `todos.update`,
    // so a direct `.set()` is appropriate here (test-only).
    todos.set({ items: [], filter: "all" });
  });

  it("addTodo appends an item with the given text", () => {
    addTodo("walk the dog");
    expect(todos.val.items.length).toBe(1);
    expect(todos.val.items[0].text).toBe("walk the dog");
    expect(todos.val.items[0].done).toBeFalsy();
  });

  it("addTodo ignores empty / whitespace-only input", () => {
    addTodo("   ");
    expect(todos.val.items.length).toBe(0);
  });

  it("toggleTodo flips done for the matching id", () => {
    addTodo("a");
    const id = todos.val.items[0].id;
    toggleTodo(id);
    expect(todos.val.items[0].done).toBeTruthy();
    toggleTodo(id);
    expect(todos.val.items[0].done).toBeFalsy();
  });

  it("deleteTodo removes the matching item", () => {
    addTodo("a");
    addTodo("b");
    const idA = todos.val.items[0].id;
    deleteTodo(idA);
    expect(todos.val.items.length).toBe(1);
    expect(todos.val.items[0].text).toBe("b");
  });

  it("editTodo updates the text", () => {
    addTodo("a");
    editTodo(todos.val.items[0].id, "renamed");
    expect(todos.val.items[0].text).toBe("renamed");
  });

  it("editTodo with empty text deletes the item", () => {
    addTodo("a");
    editTodo(todos.val.items[0].id, "   ");
    expect(todos.val.items.length).toBe(0);
  });

  it("setFilter updates the filter field only", () => {
    addTodo("a");
    setFilter("done");
    expect(todos.val.filter).toBe("done");
    expect(todos.val.items.length).toBe(1);
  });
});
