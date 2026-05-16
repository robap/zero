// Single backend client for the tracker example. Middleware is
// registered from `app.ts` (cross-cutting policy belongs to the
// composition root, not a feature store). Stores and routes import
// `api` from here and call its methods directly.

import { createHttp } from "zero/http";
import type { HttpClient } from "zero/http";

export const api: HttpClient = createHttp();
