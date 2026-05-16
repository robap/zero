// stores/todos.ts — the only module that mutates `todos`. Components import
// the mutator functions and call them; they never reach for `todos.set(...)`.

import { signal } from "zero";
import type { Signal } from "zero";
import { load, save } from "../lib/storage.ts";

export type Filter = "all" | "active" | "done";

export interface Todo {
  id: string;
  text: string;
  done: boolean;
}

export interface TodosState {
  items: Todo[];
  filter: Filter;
}

const STORAGE_KEY = "zero.todos";

const initial: TodosState = load<TodosState>(STORAGE_KEY, { items: [], filter: "all" });

export const todos: Signal<TodosState> = signal(initial);

/**
 * Persist the current state. Called from a module-load effect in app.ts so
 * every mutation is flushed to storage.
 */
export function persistTodos(): void {
  save(STORAGE_KEY, todos.val);
}

export function addTodo(text: string): void {
  const t = text.trim();
  if (!t) return;
  todos.update((s) => ({
    ...s,
    items: [...s.items, { id: _nextId(), text: t, done: false }],
  }));
}

export function toggleTodo(id: string): void {
  todos.update((s) => ({
    ...s,
    items: s.items.map((it) => (it.id === id ? { ...it, done: !it.done } : it)),
  }));
}

export function deleteTodo(id: string): void {
  todos.update((s) => ({ ...s, items: s.items.filter((it) => it.id !== id) }));
}

export function editTodo(id: string, text: string): void {
  const t = text.trim();
  if (!t) {
    deleteTodo(id);
    return;
  }
  todos.update((s) => ({
    ...s,
    items: s.items.map((it) => (it.id === id ? { ...it, text: t } : it)),
  }));
}

export function setFilter(filter: Filter): void {
  todos.update((s) => ({ ...s, filter }));
}

let _idCounter = 0;
function _nextId(): string {
  _idCounter += 1;
  return `${Date.now().toString(36)}-${_idCounter.toString(36)}`;
}
