import { html, inject } from "zero";
import type { Signal, TemplateResult } from "zero";
import { Button, Checkbox } from "zero/components";
import { Keys } from "../state.ts";
import type { Todo } from "../stores/todos.ts";
import { toggleTodo, deleteTodo } from "../stores/todos.ts";

export interface TodoItemProps {
  todo: Todo;
}

// Bridge `Signal<boolean>` for `Checkbox`. Reads `done` from the store on
// every `.val` access (which subscribes the Checkbox's reactive binding to
// the underlying store signal); `.set` and `.update` call the store mutator
// instead of writing a local value. This adapts the Checkbox component's
// signal-binding contract to a store-driven, id-keyed mutation without a
// local-signal + effect detour.
function bindToggle(id: string): Signal<boolean> {
  const read = (): boolean => {
    const state = inject(Keys.Todos).val;
    return state.items.find((t) => t.id === id)?.done ?? false;
  };
  return {
    get val() {
      return read();
    },
    set(v: boolean) {
      if (v !== read()) toggleTodo(id);
    },
    update(fn: (current: boolean) => boolean) {
      this.set(fn(read()));
    },
  };
}

export default function TodoItem({ todo }: TodoItemProps): TemplateResult {
  const done = bindToggle(todo.id);
  return html`
    <li class="todo-item cluster gap-sm" data-id=${todo.id}>
      ${Checkbox({ checked: done })}
      <span class=${todo.done ? "todo-done" : ""}>${todo.text}</span>
      ${Button({
        variant: "ghost",
        size: "sm",
        onClick: () => deleteTodo(todo.id),
        children: "Delete",
      })}
    </li>
  `;
}
