import { html, signal } from "zero";
import type { TemplateResult } from "zero";
import { Input, Button } from "zero/components";
import { addTodo } from "../stores/todos.ts";

export default function AddTodoForm(): TemplateResult {
  const text = signal("");
  const onSubmit = (e: Event) => {
    e.preventDefault();
    if (!text.val.trim()) return;
    addTodo(text.val);
    text.set("");
  };
  return html`
    <form class="add-todo cluster gap-sm" @submit=${onSubmit}>
      ${Input({ value: text, placeholder: "What needs doing?" })}
      ${Button({ children: "Add" })}
    </form>
  `;
}
