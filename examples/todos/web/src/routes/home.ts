import { html, inject, each, computed } from "zero";
import type { TemplateResult } from "zero";
import { Card } from "zero/components";
import { Keys } from "../state.ts";
import type { Filter, Todo } from "../stores/todos.ts";
import AddTodoForm from "../components/AddTodoForm.ts";
import FilterBar from "../components/FilterBar.ts";
import TodoItem from "../components/TodoItem.ts";

function applyFilter(items: Todo[], filter: Filter): Todo[] {
  if (filter === "active") return items.filter((t) => !t.done);
  if (filter === "done") return items.filter((t) => t.done);
  return items;
}

export default function Home(): TemplateResult {
  const visible = computed(() => {
    const s = inject(Keys.Todos).val;
    return applyFilter(s.items, s.filter);
  });
  return html`
    <section class="stack pad-xl">
      ${Card({
        children: html`
          <div class="stack gap-md">
            ${AddTodoForm()}
            ${FilterBar()}
            <ul class="todo-list stack gap-sm">
              ${each(visible, (todo) => TodoItem({ todo }), (todo) => todo.id)}
            </ul>
          </div>
        `,
      })}
    </section>
  `;
}
