const ELEMENT_NODE = 1;
const TEXT_NODE = 3;
const COMMENT_NODE = 8;
const DOCUMENT_FRAGMENT_NODE = 11;

/**
 * Memo of parsed complex selectors, keyed on the raw selector string.
 * @type {Map<string, Array<Array<{ combinator: string, compound: object }>>>}
 */
const _selectorCache = new Map();

/**
 * Parse a compound selector string into a descriptor object. When called as
 * part of a complex-selector split, `offset` and `fullSelector` make the
 * reported error position index into the original full string.
 * @param {string} selector the compound text to tokenize
 * @param {number} [offset] index of `selector` within `fullSelector`
 * @param {string} [fullSelector] original full selector string for error text
 * @returns {{ tag: string|null, id: string|null, classes: string[], attrs: Array<{name: string, value: string|null}> }}
 */
function _parseSelector(selector, offset = 0, fullSelector = selector) {
  if (selector === "") throw new Error("dom-shim: empty selector");
  const result = { tag: null, id: null, classes: [], attrs: [] };
  let i = 0;

  /** @param {number} pos @param {string} reason */
  function malformed(pos, reason) {
    throw new Error(`dom-shim: malformed selector "${fullSelector}" at position ${pos + offset} (${reason})`);
  }

  if (i < selector.length && /[a-zA-Z]/.test(selector[i])) {
    const start = i++;
    while (i < selector.length && /[a-zA-Z0-9]/.test(selector[i])) i++;
    result.tag = selector.slice(start, i).toLowerCase();
  }

  while (i < selector.length) {
    const ch = selector[i];
    if (ch === '#') {
      const pos = i++;
      if (result.id !== null) malformed(pos, "duplicate id");
      const start = i;
      while (i < selector.length && /[a-zA-Z0-9_-]/.test(selector[i])) i++;
      if (i === start) malformed(pos, "expected id after #");
      result.id = selector.slice(start, i);
    } else if (ch === '.') {
      const pos = i++;
      const start = i;
      while (i < selector.length && /[a-zA-Z0-9_-]/.test(selector[i])) i++;
      if (i === start) malformed(pos, "expected class name after .");
      result.classes.push(selector.slice(start, i));
    } else if (ch === '[') {
      const pos = i++;
      if (i >= selector.length || !/[a-zA-Z]/.test(selector[i])) {
        malformed(pos, "expected attribute name after [");
      }
      const nameStart = i++;
      while (i < selector.length && /[a-zA-Z0-9_:-]/.test(selector[i])) i++;
      const name = selector.slice(nameStart, i).toLowerCase();
      if (i >= selector.length) malformed(pos, "unclosed attribute bracket");
      if (selector[i] === ']') {
        i++;
        result.attrs.push({ name, value: null });
      } else if (selector[i] === '=') {
        i++;
        if (i >= selector.length) malformed(i - 1, "expected value after =");
        let value;
        if (selector[i] === '"') {
          i++;
          const valStart = i;
          while (i < selector.length && selector[i] !== '"') i++;
          if (i >= selector.length) malformed(pos, "unclosed attribute bracket");
          value = selector.slice(valStart, i);
          i++;
          if (i >= selector.length || selector[i] !== ']') malformed(i, "expected ] after attribute value");
          i++;
        } else if (selector[i] === "'") {
          i++;
          const valStart = i;
          while (i < selector.length && selector[i] !== "'") i++;
          if (i >= selector.length) malformed(pos, "unclosed attribute bracket");
          value = selector.slice(valStart, i);
          i++;
          if (i >= selector.length || selector[i] !== ']') malformed(i, "expected ] after attribute value");
          i++;
        } else {
          const valStart = i;
          while (i < selector.length && selector[i] !== ']') i++;
          if (i >= selector.length) malformed(pos, "unclosed attribute bracket");
          value = selector.slice(valStart, i);
          i++;
        }
        result.attrs.push({ name, value });
      } else {
        malformed(i, `unexpected character '${selector[i]}' in attribute selector`);
      }
    } else {
      malformed(i, `unexpected character '${ch}'`);
    }
  }

  return result;
}

/**
 * Scan a complex selector into a list of branches, each branch an array of
 * step descriptors `{ combinator, start, end }` bounding the compound text
 * within `selector`. `combinator` is `"none"` for the first step of a branch,
 * else `"descendant"` or `"child"`. Throws `dom-shim: malformed selector …`
 * on dangling/leading combinators and empty list branches.
 * @param {string} selector
 * @returns {Array<Array<{ combinator: string, start: number, end: number }>>}
 */
function _splitSelector(selector) {
  /** @param {number} pos @param {string} reason */
  function malformed(pos, reason) {
    throw new Error(`dom-shim: malformed selector "${selector}" at position ${pos} (${reason})`);
  }

  /** @type {Array<Array<{ combinator: string, start: number, end: number }>>} */
  const branches = [];
  /** @type {Array<{ combinator: string, start: number, end: number }>} */
  let branch = [];
  let pendingCombinator = "none";
  let compoundStart = -1;
  let bracketDepth = 0;
  let quote = "";

  /** @param {number} end */
  function closeCompound(end) {
    branch.push({ combinator: pendingCombinator, start: compoundStart, end });
    compoundStart = -1;
    pendingCombinator = "none";
  }

  /** @param {number} pos */
  function closeBranch(pos) {
    if (compoundStart !== -1) closeCompound(pos);
    if (pendingCombinator === "child") malformed(pos, "expected selector after >");
    if (branch.length === 0) malformed(pos, "empty selector in list");
    branches.push(branch);
    branch = [];
    pendingCombinator = "none";
  }

  for (let i = 0; i < selector.length; i++) {
    const ch = selector[i];
    if (quote) {
      if (ch === quote) quote = "";
      continue;
    }
    if (bracketDepth > 0) {
      if (ch === '"' || ch === "'") quote = ch;
      else if (ch === "]") bracketDepth--;
      continue;
    }
    if (ch === "[") {
      bracketDepth++;
      if (compoundStart === -1) compoundStart = i;
      continue;
    }
    if (ch === ",") {
      closeBranch(i);
      continue;
    }
    if (ch === " " || ch === "\t" || ch === "\n" || ch === "\r" || ch === "\f") {
      if (compoundStart !== -1) {
        closeCompound(i);
        pendingCombinator = "descendant";
      }
      continue;
    }
    if (ch === ">") {
      if (compoundStart !== -1) closeCompound(i);
      if (branch.length === 0) malformed(i, "expected selector before >");
      if (pendingCombinator === "child") malformed(i, "expected selector after >");
      pendingCombinator = "child";
      continue;
    }
    if (compoundStart === -1) compoundStart = i;
  }
  closeBranch(selector.length);

  return branches;
}

/**
 * Parse a complex selector (comma list of combinator-joined compounds) into a
 * list of branches, each branch an array of `{ combinator, compound }` steps.
 * Memoized on the raw selector string.
 * @param {string} selector
 * @returns {Array<Array<{ combinator: string, compound: object }>>}
 */
function _parseComplexSelector(selector) {
  if (selector.trim() === "") throw new Error("dom-shim: empty selector");
  const cached = _selectorCache.get(selector);
  if (cached) return cached;
  const branches = _splitSelector(selector);
  const parsed = branches.map(branch =>
    branch.map(({ combinator, start, end }) => ({
      combinator,
      compound: _parseSelector(selector.slice(start, end), start, selector),
    })),
  );
  _selectorCache.set(selector, parsed);
  return parsed;
}

/**
 * Test a node against a single parsed compound descriptor.
 * @param {object} node
 * @param {{ tag: string|null, id: string|null, classes: string[], attrs: Array<{name: string, value: string|null}> }} parsed
 * @returns {boolean}
 */
function _matchCompound(node, parsed) {
  if (node.nodeType !== ELEMENT_NODE) return false;
  if (parsed.tag != null) {
    if (node.tagName == null || node.tagName.toLowerCase() !== parsed.tag) return false;
  }
  if (parsed.id != null) {
    if (!node.getAttribute || node.getAttribute('id') !== parsed.id) return false;
  }
  if (parsed.classes.length > 0) {
    const cls = node.getAttribute ? node.getAttribute('class') : null;
    if (cls == null) return false;
    const tokens = cls.split(/\s+/).filter(Boolean);
    for (const c of parsed.classes) if (!tokens.includes(c)) return false;
  }
  for (const { name, value } of parsed.attrs) {
    if (!node.hasAttribute || !node.hasAttribute(name)) return false;
    if (value != null && node.getAttribute(name) !== value) return false;
  }
  return true;
}

/**
 * Recurse leftward through a branch's steps, matching the ancestor chain of
 * `node`. `steps[i]` is the step whose compound `node` already satisfies; the
 * combinator on `steps[i]` links `steps[i-1]` (an ancestor) to `node`.
 * Ancestor walking is bounded by `root` (inclusive); a null/undefined `root`
 * walks to the top of the tree.
 * @param {object} node
 * @param {Array<{ combinator: string, compound: object }>} steps
 * @param {number} i
 * @param {object|null|undefined} root
 * @returns {boolean}
 */
function _matchFrom(node, steps, i, root) {
  if (i === 0) return true;
  const { combinator, compound } = steps[i];
  const left = steps[i - 1].compound;
  if (combinator === "child") {
    if (node === root) return false; // parent would be outside the root bound
    const p = node.parentNode;
    if (!p) return false;
    if (_matchCompound(p, left) && _matchFrom(p, steps, i - 1, root)) return true;
    return false;
  }
  // descendant
  let anc = node === root ? null : node.parentNode;
  while (anc) {
    if (_matchCompound(anc, left) && _matchFrom(anc, steps, i - 1, root)) return true;
    if (anc === root) break;
    anc = anc.parentNode;
  }
  return false;
}

/**
 * Test a node against a parsed complex selector (list of branches). A node
 * matches if it matches any branch: its rightmost compound matches the node
 * and the preceding compounds match an ancestor chain (right-to-left), bounded
 * by `root` (inclusive; null/undefined ⇒ unbounded).
 * @param {object} node
 * @param {Array<Array<{ combinator: string, compound: object }>>} parsedList
 * @param {object|null|undefined} root
 * @returns {boolean}
 */
function _matchComplexSelector(node, parsedList, root) {
  for (const steps of parsedList) {
    const last = steps.length - 1;
    if (!_matchCompound(node, steps[last].compound)) continue;
    if (_matchFrom(node, steps, last, root)) return true;
  }
  return false;
}

function _walkDescendants(root, fn) {
  for (const child of root.childNodes || []) {
    fn(child);
    _walkDescendants(child, fn);
  }
}

function _applySiblingGetters(node, getParent) {
  Object.defineProperties(node, {
    nextSibling: {
      get() {
        const p = getParent();
        if (!p) return null;
        const i = p.childNodes.indexOf(node);
        return i === -1 || i === p.childNodes.length - 1 ? null : p.childNodes[i + 1];
      },
      configurable: true,
    },
    previousSibling: {
      get() {
        const p = getParent();
        if (!p) return null;
        const i = p.childNodes.indexOf(node);
        return i <= 0 ? null : p.childNodes[i - 1];
      },
      configurable: true,
    },
    firstChild: {
      get() { return node.childNodes ? node.childNodes[0] ?? null : null; },
      configurable: true,
    },
    lastChild: {
      get() { return node.childNodes ? node.childNodes[node.childNodes.length - 1] ?? null : null; },
      configurable: true,
    },
  });
}

function _appendChild(parent, child) {
  if (child.nodeType === DOCUMENT_FRAGMENT_NODE) {
    const children = [...child.childNodes];
    for (const c of children) _appendChild(parent, c);
    return child;
  }
  if (child.parentNode) _removeChild(child.parentNode, child);
  parent.childNodes.push(child);
  child.parentNode = parent;
  return child;
}

function _insertBefore(parent, child, ref) {
  if (ref == null) return _appendChild(parent, child);
  if (child.nodeType === DOCUMENT_FRAGMENT_NODE) {
    const children = [...child.childNodes];
    for (const c of children) _insertBefore(parent, c, ref);
    return child;
  }
  if (child.parentNode) _removeChild(child.parentNode, child);
  const i = parent.childNodes.indexOf(ref);
  if (i === -1) throw new Error('dom-shim: ref node not a child of parent');
  parent.childNodes.splice(i, 0, child);
  child.parentNode = parent;
  return child;
}

function _removeChild(parent, child) {
  const i = parent.childNodes.indexOf(child);
  if (i === -1) throw new Error('dom-shim: child not found in parent');
  parent.childNodes.splice(i, 1);
  child.parentNode = null;
  return child;
}

function _cloneNode(node, deep) {
  let clone;
  if (node.nodeType === ELEMENT_NODE) {
    clone = createElement(node.tagName.toLowerCase());
    for (const [k, v] of node.attributes) clone.attributes.set(k, v);
  } else if (node.nodeType === TEXT_NODE) {
    clone = createTextNode(node.nodeValue);
  } else if (node.nodeType === COMMENT_NODE) {
    clone = createComment(node.data);
  } else {
    clone = createDocumentFragment();
  }
  if (deep && node.childNodes) {
    for (const child of node.childNodes) _appendChild(clone, _cloneNode(child, true));
  }
  return clone;
}

/**
 * Resolve an `addEventListener` `options` argument (boolean or object) into a
 * `{capture, once}` record. Extracted into a top-level helper so the branch
 * doesn't sit inline with `createElement` (see boa-maplock-finalizer note).
 * @internal
 * @param {boolean|object|undefined} options
 * @returns {{ capture: boolean, once: boolean }}
 */
function _normalizeListenerOptions(options) {
  if (typeof options === "boolean") return { capture: options, once: false };
  const opts = options || {};
  return { capture: Boolean(opts.capture), once: Boolean(opts.once) };
}

/**
 * Resolve a `removeEventListener` `options` argument (boolean or object) to a
 * `capture` flag.
 * @internal
 * @param {boolean|object|undefined} options
 * @returns {boolean}
 */
function _resolveRemoveCapture(options) {
  if (typeof options === "boolean") return options;
  return Boolean(options && options.capture);
}

/**
 * Append a listener entry to `el._listeners` keyed by `event`.
 * @internal
 * @param {object} el
 * @param {string} event
 * @param {Function} handler
 * @param {boolean|object|undefined} options
 * @returns {void}
 */
function _addListener(el, event, handler, options) {
  const { capture, once } = _normalizeListenerOptions(options);
  if (!el._listeners.has(event)) el._listeners.set(event, []);
  el._listeners.get(event).push({ handler, once, capture });
}

/**
 * Remove a listener entry from `el._listeners` matching `handler` and capture
 * flag.
 * @internal
 * @param {object} el
 * @param {string} event
 * @param {Function} handler
 * @param {boolean|object|undefined} options
 * @returns {void}
 */
function _removeListener(el, event, handler, options) {
  if (!el._listeners.has(event)) return;
  const wantCapture = _resolveRemoveCapture(options);
  const list = el._listeners
    .get(event)
    .filter(e => !(e.handler === handler && e.capture === wantCapture));
  el._listeners.set(event, list);
}

/**
 * Decorate a legacy plain-object event (no preventDefault/stopPropagation) with
 * the methods the dispatch walker reads from. Allows callers that still pass
 * `{type, ...}` bag-of-fields to round-trip through dispatchEvent.
 * @internal
 * @param {object} event
 * @returns {object} the same object, mutated
 */
function _ensureEventShape(event) {
  if (typeof event.preventDefault !== "function") {
    event.preventDefault = () => { event.defaultPrevented = true; };
  }
  if (typeof event.stopPropagation !== "function") {
    event.stopPropagation = () => { event._stopPropagation = true; };
  }
  if (typeof event.stopImmediatePropagation !== "function") {
    event.stopImmediatePropagation = () => {
      event._stopPropagation = true;
      event._stopImmediate = true;
    };
  }
  if (event._stopPropagation == null) event._stopPropagation = false;
  if (event._stopImmediate == null) event._stopImmediate = false;
  if (event.defaultPrevented == null) event.defaultPrevented = false;
  return event;
}

/**
 * Fire listeners registered on `node` for `event.type` whose `capture` flag
 * matches `capture`. Respects stopImmediatePropagation and removes
 * `{once: true}` entries after firing.
 * @internal
 * @param {object} node
 * @param {object} event
 * @param {boolean} capture
 * @returns {void}
 */
function _fireListenersOn(node, event, capture) {
  const listeners = node._listeners;
  if (!listeners) return;
  const list = listeners.get(event.type);
  if (!list) return;
  event.currentTarget = node;
  const snapshot = [...list];
  for (const entry of snapshot) {
    if (entry.capture !== capture) continue;
    if (event._stopImmediate) break;
    entry.handler.call(node, event);
    if (entry.once) {
      const idx = list.indexOf(entry);
      if (idx >= 0) list.splice(idx, 1);
    }
  }
}

/**
 * Dispatch `event` on `target` using real-DOM capture/target/bubble ordering.
 * Returns `false` only when `event.cancelable && event.defaultPrevented`.
 * @internal
 * @param {object} target
 * @param {object} event
 * @returns {boolean}
 */
function _dispatchEvent(target, event) {
  _ensureEventShape(event);
  if (event.target == null) event.target = target;

  const path = [];
  let n = target;
  while (n) { path.push(n); n = n.parentNode; }

  event.eventPhase = 1;
  for (let i = path.length - 1; i >= 1; i--) {
    if (event._stopPropagation) break;
    _fireListenersOn(path[i], event, true);
  }
  if (!event._stopPropagation) {
    event.eventPhase = 2;
    _fireListenersOn(target, event, false);
    if (!event._stopImmediate) _fireListenersOn(target, event, true);
  }
  if (event.bubbles && !event._stopPropagation) {
    event.eventPhase = 3;
    for (let i = 1; i < path.length; i++) {
      if (event._stopPropagation) break;
      _fireListenersOn(path[i], event, false);
    }
  }
  event.eventPhase = 0;
  event.currentTarget = null;
  return !(event.cancelable && event.defaultPrevented);
}

/**
 * Set `name` on `el`'s attribute map, special-casing `style` so the style map
 * stays in sync.
 * @internal
 * @param {object} el
 * @param {string} name
 * @param {unknown} value
 * @returns {void}
 */
function _setAttribute(el, name, value) {
  if (name === "style") {
    _writeStyleAttribute(el, value);
    return;
  }
  el.attributes.set(name, String(value));
}

/**
 * Read `name` from `el`'s attribute map; for `style` serialize the style map.
 * @internal
 * @param {object} el
 * @param {string} name
 * @returns {string|null}
 */
function _getAttribute(el, name) {
  if (name === "style") return _readStyleAttribute(el);
  return el.attributes.has(name) ? el.attributes.get(name) : null;
}

/**
 * `hasAttribute` for `el`, special-casing `style` to consider the live style
 * map.
 * @internal
 * @param {object} el
 * @param {string} name
 * @returns {boolean}
 */
function _hasAttribute(el, name) {
  if (name === "style") {
    return (el._styleMap && el._styleMap.size > 0) || el.attributes.has("style");
  }
  return el.attributes.has(name);
}

/**
 * Remove `name` from `el`'s attribute map and clear the style map when needed.
 * @internal
 * @param {object} el
 * @param {string} name
 * @returns {void}
 */
function _removeAttribute(el, name) {
  if (name === "style" && el._styleMap) el._styleMap.clear();
  el.attributes.delete(name);
}

/**
 * Read the element's current class attribute as a token array.
 * @internal
 * @param {object} el
 * @returns {string[]}
 */
function _classTokens(el) {
  const v = el.getAttribute("class");
  if (v == null) return [];
  return v.split(/\s+/).filter(Boolean);
}

/**
 * Write a token array back to the class attribute (or remove the attribute
 * when empty).
 * @internal
 * @param {object} el
 * @param {string[]} tokens
 * @returns {void}
 */
function _writeClassTokens(el, tokens) {
  if (tokens.length === 0) {
    el.removeAttribute("class");
  } else {
    el.setAttribute("class", tokens.join(" "));
  }
}

/**
 * Build a DOMTokenList-shaped object backed by the element's class attribute.
 * @internal
 * @param {object} el
 * @returns {object}
 */
function _makeClassList(el) {
  return {
    add(...names) {
      const t = _classTokens(el);
      for (const n of names) if (!t.includes(n)) t.push(n);
      _writeClassTokens(el, t);
    },
    remove(...names) {
      const t = _classTokens(el).filter(x => !names.includes(x));
      _writeClassTokens(el, t);
    },
    toggle(name, force) {
      const t = _classTokens(el);
      const has = t.includes(name);
      const shouldHave = force === undefined ? !has : Boolean(force);
      if (shouldHave && !has) {
        t.push(name);
        _writeClassTokens(el, t);
        return true;
      }
      if (!shouldHave && has) {
        _writeClassTokens(el, t.filter(x => x !== name));
        return false;
      }
      return shouldHave;
    },
    contains(name) {
      return _classTokens(el).includes(name);
    },
    replace(oldName, newName) {
      const t = _classTokens(el);
      const i = t.indexOf(oldName);
      if (i < 0) return false;
      t[i] = newName;
      _writeClassTokens(el, t);
      return true;
    },
    get length() { return _classTokens(el).length; },
    item(i) { return _classTokens(el)[i] ?? null; },
  };
}

/**
 * Convert a camelCase identifier to its kebab-case form.
 * @internal
 * @param {string} s
 * @returns {string}
 */
function _camelToKebab(s) {
  return s.replace(/[A-Z]/g, c => "-" + c.toLowerCase());
}

/**
 * Convert a kebab-case identifier to its camelCase form.
 * @internal
 * @param {string} s
 * @returns {string}
 */
function _kebabToCamel(s) {
  return s.replace(/-([a-z])/g, (_, c) => c.toUpperCase());
}

/**
 * Build a DOMStringMap-shaped Proxy backed by the element's `data-*`
 * attributes.
 * @internal
 * @param {object} el
 * @returns {object}
 */
function _makeDataset(el) {
  return new Proxy({}, {
    get(_target, key) {
      if (typeof key !== "string") return undefined;
      const attr = "data-" + _camelToKebab(key);
      const v = el.getAttribute(attr);
      return v == null ? undefined : v;
    },
    set(_target, key, value) {
      if (typeof key !== "string") return false;
      el.setAttribute("data-" + _camelToKebab(key), String(value));
      return true;
    },
    has(_target, key) {
      if (typeof key !== "string") return false;
      return el.hasAttribute("data-" + _camelToKebab(key));
    },
    deleteProperty(_target, key) {
      if (typeof key !== "string") return true;
      el.removeAttribute("data-" + _camelToKebab(key));
      return true;
    },
    ownKeys() {
      const keys = [];
      for (const k of el.attributes.keys()) {
        if (k.startsWith("data-")) keys.push(_kebabToCamel(k.slice(5)));
      }
      return keys;
    },
    getOwnPropertyDescriptor(_target, key) {
      if (typeof key !== "string") return undefined;
      const attr = "data-" + _camelToKebab(key);
      if (!el.hasAttribute(attr)) return undefined;
      return {
        configurable: true,
        enumerable: true,
        writable: true,
        value: el.getAttribute(attr),
      };
    },
  });
}

/**
 * Serialize a style map to a CSS declaration string.
 * @internal
 * @param {Map<string, string>} map
 * @returns {string}
 */
function _styleSerialize(map) {
  const parts = [];
  for (const [k, v] of map) parts.push(`${k}: ${v}`);
  return parts.join("; ");
}

/**
 * Parse a CSS declaration string into the given style map (clears it first).
 * Known limitation: ignores quoted values and !important; the component
 * surface in the repo doesn't use these.
 * @internal
 * @param {Map<string, string>} map
 * @param {string} str
 * @returns {void}
 */
function _styleParseInto(map, str) {
  map.clear();
  if (!str) return;
  for (const decl of str.split(";")) {
    const idx = decl.indexOf(":");
    if (idx < 0) continue;
    const name = decl.slice(0, idx).trim();
    const value = decl.slice(idx + 1).trim();
    if (name) map.set(name, value);
  }
}

/**
 * Resolve a property key (camelCase or `--custom`) to its CSS property name.
 * @internal
 * @param {string} key
 * @returns {string}
 */
function _styleKeyToProp(key) {
  if (key.startsWith("--")) return key;
  return _camelToKebab(key);
}

/**
 * Build a CSSStyleDeclaration-shaped Proxy backed by an internal Map. Style
 * mutations write into the map; `el.getAttribute('style')` reads back via
 * `_styleSerialize`.
 * @internal
 * @param {object} el
 * @param {Map<string, string>} map
 * @returns {object}
 */
function _makeStyle(el, map) {
  const api = {
    setProperty(name, value) { map.set(String(name), String(value)); },
    removeProperty(name) { map.delete(String(name)); },
    getPropertyValue(name) { return map.get(String(name)) ?? ""; },
    get cssText() { return _styleSerialize(map); },
    set cssText(v) { _styleParseInto(map, String(v)); },
  };
  return new Proxy(api, {
    get(target, key) {
      if (typeof key === "string" && !(key in target)) {
        return map.get(_styleKeyToProp(key)) ?? "";
      }
      return target[key];
    },
    set(target, key, value) {
      if (typeof key === "string" && !(key in target)) {
        map.set(_styleKeyToProp(key), String(value));
        return true;
      }
      target[key] = value;
      return true;
    },
  });
  // `el` is referenced for future enhancements (e.g. firing mutation hooks).
  // Leaving it unused is intentional for now.
  // eslint-disable-next-line no-unreachable
  void el;
}

/**
 * Read concatenated text content of all descendant text nodes.
 * @internal
 * @param {object} el
 * @returns {string}
 */
function _readTextContent(el) {
  let out = "";
  (function walk(node) {
    if (node.nodeType === TEXT_NODE) out += node.nodeValue;
    if (node.childNodes) for (const c of node.childNodes) walk(c);
  })(el);
  return out;
}

/**
 * Remove every child of `el` then append a single text node with `text` (when
 * non-empty).
 * @internal
 * @param {object} el
 * @param {string} text
 * @returns {void}
 */
function _writeTextContent(el, text) {
  while (el.childNodes.length > 0) _removeChild(el, el.childNodes[0]);
  if (text !== "") _appendChild(el, createTextNode(text));
}

/**
 * Define `name` as a string property on `el` coupled to attribute `attrName`.
 * @internal
 * @param {object} el
 * @param {string} prop
 * @param {string} attrName
 * @returns {void}
 */
function _defineStringAttrProp(el, prop, attrName) {
  Object.defineProperty(el, prop, {
    get() { return _getAttribute(el, attrName) ?? ""; },
    set(v) { _setAttribute(el, attrName, v == null ? "" : String(v)); },
    configurable: true,
    enumerable: true,
  });
}

/**
 * Define `value` as a live property with a browser-like default-vs-live split:
 * the getter returns the live value once set, else the `value` attribute (the
 * default); the setter stores the live value and moves the caret to the end
 * (as a browser does on assignment). A late `setAttribute("value", …)` updates
 * only the default and cannot change what `.value` returns after a set.
 * @internal
 * @param {object} el
 * @returns {void}
 */
function _defineLiveValueProp(el) {
  Object.defineProperty(el, "value", {
    get() { return el._liveValue !== undefined ? el._liveValue : (_getAttribute(el, "value") ?? ""); },
    set(v) {
      el._liveValue = v == null ? "" : String(v);
      const len = el._liveValue.length;
      el._selStart = len;
      el._selEnd = len;
    },
    configurable: true,
    enumerable: true,
  });
}

/**
 * Define `checked` as a live property with the same default-vs-live split as
 * `_defineLiveValueProp`: the getter returns the live boolean once set, else
 * the `checked` attribute presence (the default). A late
 * `setAttribute("checked", …)` updates only the default.
 * @internal
 * @param {object} el
 * @returns {void}
 */
function _defineLiveCheckedProp(el) {
  Object.defineProperty(el, "checked", {
    get() { return el._liveChecked !== undefined ? el._liveChecked : _hasAttribute(el, "checked"); },
    set(v) { el._liveChecked = !!v; },
    configurable: true,
    enumerable: true,
  });
}

/**
 * Define `name` as a boolean property on `el` coupled to attribute presence.
 * @internal
 * @param {object} el
 * @param {string} prop
 * @param {string} attrName
 * @returns {void}
 */
function _defineBoolAttrProp(el, prop, attrName) {
  Object.defineProperty(el, prop, {
    get() { return _hasAttribute(el, attrName); },
    set(v) {
      if (v) _setAttribute(el, attrName, "");
      else _removeAttribute(el, attrName);
    },
    configurable: true,
    enumerable: true,
  });
}

/**
 * @internal
 * @param {object} el
 * @returns {void}
 */
function _attachClassList(el) {
  Object.defineProperty(el, "classList", {
    value: _makeClassList(el),
    configurable: true,
  });
}

/**
 * @internal
 * @param {object} el
 * @returns {void}
 */
function _attachDataset(el) {
  Object.defineProperty(el, "dataset", {
    value: _makeDataset(el),
    configurable: true,
  });
}

/**
 * @internal
 * @param {object} el
 * @returns {void}
 */
function _attachStyle(el) {
  const styleMap = new Map();
  el._styleMap = styleMap;
  Object.defineProperty(el, "style", {
    value: _makeStyle(el, styleMap),
    configurable: true,
  });
}

/**
 * @internal
 * @param {object} el
 * @returns {void}
 */
function _attachTextContent(el) {
  Object.defineProperty(el, "textContent", {
    get() { return _readTextContent(el); },
    set(v) { _writeTextContent(el, v == null ? "" : String(v)); },
    configurable: true,
    enumerable: true,
  });
}

/**
 * @internal
 * @param {object} el
 * @returns {void}
 */
function _attachClassNameProp(el) {
  Object.defineProperty(el, "className", {
    get() { return _getAttribute(el, "class") ?? ""; },
    set(v) { _setAttribute(el, "class", v == null ? "" : String(v)); },
    configurable: true,
    enumerable: true,
  });
}

/**
 * @internal
 * @param {object} el
 * @returns {void}
 */
function _attachInputProps(el) {
  // Form controls track a live property that diverges from the content
  // attribute (the default); other elements keep simple attribute reflection.
  if (el.tagName === "INPUT" || el.tagName === "TEXTAREA") _defineLiveValueProp(el);
  else _defineStringAttrProp(el, "value", "value");
  if (el.tagName === "INPUT") _defineLiveCheckedProp(el);
  else _defineBoolAttrProp(el, "checked", "checked");
  _defineBoolAttrProp(el, "disabled", "disabled");
  _defineBoolAttrProp(el, "selected", "selected");
  _defineStringAttrProp(el, "name", "name");
  _defineStringAttrProp(el, "type", "type");
  _defineStringAttrProp(el, "placeholder", "placeholder");
  _defineStringAttrProp(el, "htmlFor", "for");
  _attachInputSelection(el);
}

/**
 * Attach `selectionStart` / `selectionEnd` (numeric, default 0) and
 * `setSelectionRange()` on `el`. Mirrors the HTMLInputElement selection
 * surface that the Combobox component depends on at runtime and under
 * `zero test`.
 * @internal
 * @param {object} el
 * @returns {void}
 */
function _attachInputSelection(el) {
  el._selStart = 0;
  el._selEnd = 0;
  Object.defineProperty(el, "selectionStart", {
    get() { return el._selStart; },
    set(v) { el._selStart = Number.isFinite(+v) ? +v : 0; },
    configurable: true,
    enumerable: true,
  });
  Object.defineProperty(el, "selectionEnd", {
    get() { return el._selEnd; },
    set(v) { el._selEnd = Number.isFinite(+v) ? +v : 0; },
    configurable: true,
    enumerable: true,
  });
  el.setSelectionRange = _setSelectionRange;
}

/**
 * `setSelectionRange` shared method body. Lives at module scope rather
 * than as a per-element closure so it cannot accumulate per-element
 * captures across the Boa heap.
 * @internal
 * @param {number} start
 * @param {number} end
 * @returns {void}
 */
function _setSelectionRange(start, end) {
  const len = (this.value ?? "").length;
  const s = Number.isFinite(+start) ? +start : 0;
  const e = Number.isFinite(+end) ? +end : 0;
  this._selStart = Math.min(Math.max(0, s), len);
  this._selEnd = Math.min(Math.max(0, e), len);
}

/**
 * Collect all `<option>` descendants of `select` in document order,
 * including options nested inside `<optgroup>`.
 * @internal
 * @param {object} select
 * @returns {object[]}
 */
function _optionsOf(select) {
  const options = [];
  _walkDescendants(select, node => {
    if (node.nodeType === ELEMENT_NODE && node.tagName === "OPTION") options.push(node);
  });
  return options;
}

/**
 * Read an option's value: the `value` attribute when present, else the
 * option's text content (browser fallback).
 * @internal
 * @param {object} option
 * @returns {string}
 */
function _optionValue(option) {
  return _getAttribute(option, "value") ?? _readTextContent(option);
}

/**
 * Index of the first option of `select` carrying a `selected` attribute.
 * When none is marked: `0` for a non-`multiple` select with at least one
 * option (the browser's default-first rule), else `-1`. The default rule is
 * suppressed while the select's selection was explicitly cleared (see
 * `_setSelectedIndex`) — mirroring the browser, where clearing via the value
 * or selectedIndex setter does not re-select the first option.
 * @internal
 * @param {object} select
 * @returns {number}
 */
function _selectedIndexOf(select) {
  const options = _optionsOf(select);
  for (let i = 0; i < options.length; i++) {
    if (_hasAttribute(options[i], "selected")) return i;
  }
  if (select._selectionCleared) return -1;
  return !_hasAttribute(select, "multiple") && options.length > 0 ? 0 : -1;
}

/**
 * Make the option at index `i` the sole selection of `select`: remove the
 * `selected` attribute from every option, then mark `options[i]` when `i`
 * is in range. `-1` (or any out-of-range index) clears all selection and
 * raises the select's `_selectionCleared` flag so the default-first rule
 * stays suppressed until an option is marked again.
 * @internal
 * @param {object} select
 * @param {number} i
 * @returns {void}
 */
function _setSelectedIndex(select, i) {
  const options = _optionsOf(select);
  for (const o of options) _removeAttribute(o, "selected");
  const inRange = i >= 0 && i < options.length;
  if (inRange) _setAttribute(options[i], "selected", "");
  select._selectionCleared = !inRange;
}

/**
 * Attach the HTMLSelectElement-specific surface on top of the generic input
 * props (legal because those are defined `configurable: true`). `value` and
 * `selectedIndex` derive from the options' `selected` attributes — the shim
 * treats the attribute as current state (no default-vs-dirty model). The
 * `value` setter is strict: no matching option clears the selection, and no
 * `value` attribute is ever written on the select itself.
 * @internal
 * @param {object} el
 * @returns {void}
 */
function _attachSelectProps(el) {
  el._selectionCleared = false;
  Object.defineProperty(el, "value", {
    get() {
      const i = _selectedIndexOf(el);
      return i === -1 ? "" : _optionValue(_optionsOf(el)[i]);
    },
    set(v) {
      const wanted = String(v);
      _setSelectedIndex(el, _optionsOf(el).findIndex(o => _optionValue(o) === wanted));
    },
    configurable: true,
    enumerable: true,
  });
  Object.defineProperty(el, "selectedIndex", {
    get() { return _selectedIndexOf(el); },
    set(v) { _setSelectedIndex(el, Number.isFinite(+v) ? +v : -1); },
    configurable: true,
    enumerable: true,
  });
  Object.defineProperty(el, "options", {
    get() { return _optionsOf(el); },
    configurable: true,
    enumerable: true,
  });
  Object.defineProperty(el, "selectedOptions", {
    get() {
      const options = _optionsOf(el);
      const marked = options.filter(o => _hasAttribute(o, "selected"));
      if (marked.length > 0) return marked;
      const i = _selectedIndexOf(el);
      return i === -1 ? [] : [options[i]];
    },
    configurable: true,
    enumerable: true,
  });
  _defineBoolAttrProp(el, "multiple", "multiple");
}

/**
 * Walk `option`'s ancestors (through `<optgroup>`) to the nearest `<select>`.
 * @internal
 * @param {object} option
 * @returns {object|null}
 */
function _ownerSelect(option) {
  let node = option.parentNode;
  while (node && node.nodeType === ELEMENT_NODE && node.tagName === "OPTGROUP") {
    node = node.parentNode;
  }
  return node && node.nodeType === ELEMENT_NODE && node.tagName === "SELECT" ? node : null;
}

/**
 * Attach the HTMLOptionElement-specific surface on top of the generic input
 * props. `value` gains the browser's text-content fallback when the `value`
 * attribute is absent; `selected` reports *current* selectedness, including
 * the owning select's default-first rule. Orphan options (no `<select>`
 * ancestor) fall back to attribute presence.
 * @internal
 * @param {object} el
 * @returns {void}
 */
function _attachOptionProps(el) {
  Object.defineProperty(el, "value", {
    get() { return _optionValue(el); },
    set(v) { _setAttribute(el, "value", v == null ? "" : String(v)); },
    configurable: true,
    enumerable: true,
  });
  Object.defineProperty(el, "selected", {
    get() {
      if (_hasAttribute(el, "selected")) return true;
      const select = _ownerSelect(el);
      if (!select) return false;
      const i = _selectedIndexOf(select);
      return i !== -1 && _optionsOf(select)[i] === el;
    },
    set(v) {
      if (!v) {
        _removeAttribute(el, "selected");
        return;
      }
      const select = _ownerSelect(el);
      if (select && !_hasAttribute(select, "multiple")) {
        for (const o of _optionsOf(select)) {
          if (o !== el) _removeAttribute(o, "selected");
        }
      }
      _setAttribute(el, "selected", "");
      if (select) select._selectionCleared = false;
    },
    configurable: true,
    enumerable: true,
  });
  Object.defineProperty(el, "index", {
    get() {
      const select = _ownerSelect(el);
      return select ? _optionsOf(select).indexOf(el) : 0;
    },
    configurable: true,
    enumerable: true,
  });
}

/**
 * Attach the element property surface (classList, dataset, style, textContent,
 * className, input-shaped properties) onto `el`. Each group lives in its own
 * helper to keep `createElement`'s body trim — see boa-maplock-finalizer notes.
 * Select and option elements gain tag-specific overrides on top of the
 * generic props.
 * @internal
 * @param {object} el
 * @returns {void}
 */
function _attachElementProps(el) {
  _attachClassList(el);
  _attachDataset(el);
  _attachStyle(el);
  _attachTextContent(el);
  _attachClassNameProp(el);
  _attachInputProps(el);
  if (el.tagName === "SELECT") _attachSelectProps(el);
  else if (el.tagName === "OPTION") _attachOptionProps(el);
}

/**
 * Read the `style` attribute by serializing the element's style map.
 * @internal
 * @param {object} el
 * @returns {string|null}
 */
function _readStyleAttribute(el) {
  if (!el._styleMap || el._styleMap.size === 0) {
    return el.attributes.has("style") ? el.attributes.get("style") : null;
  }
  return _styleSerialize(el._styleMap);
}

/**
 * Write the `style` attribute by parsing it into the element's style map.
 * @internal
 * @param {object} el
 * @param {string} value
 * @returns {void}
 */
function _writeStyleAttribute(el, value) {
  el.attributes.set("style", String(value));
  if (el._styleMap) _styleParseInto(el._styleMap, String(value));
}

function createElement(tagName) {
  const el = {
    nodeType: ELEMENT_NODE,
    tagName: tagName.toUpperCase(),
    namespaceURI: 'http://www.w3.org/1999/xhtml',
    get nodeName() { return this.tagName; },
    attributes: new Map(),
    childNodes: [],
    parentNode: null,
    _listeners: new Map(),

    setAttribute(name, value) { _setAttribute(this, name, value); },
    removeAttribute(name) { _removeAttribute(this, name); },
    getAttribute(name) { return _getAttribute(this, name); },
    hasAttribute(name) { return _hasAttribute(this, name); },

    addEventListener(event, handler, options) {
      _addListener(this, event, handler, options);
    },
    removeEventListener(event, handler, options) {
      _removeListener(this, event, handler, options);
    },
    dispatchEvent(event) {
      return _dispatchEvent(this, event);
    },

    querySelector(selector) {
      const parsed = _parseComplexSelector(selector);
      let found = null;
      _walkDescendants(this, node => {
        if (!found && node.nodeType === ELEMENT_NODE && _matchComplexSelector(node, parsed, this)) found = node;
      });
      return found;
    },
    querySelectorAll(selector) {
      const parsed = _parseComplexSelector(selector);
      const results = [];
      _walkDescendants(this, node => {
        if (node.nodeType === ELEMENT_NODE && _matchComplexSelector(node, parsed, this)) results.push(node);
      });
      return results;
    },
    closest(selector) {
      const parsed = _parseComplexSelector(selector);
      let node = this;
      while (node && node.nodeType === ELEMENT_NODE) {
        if (_matchComplexSelector(node, parsed, null)) return node;
        node = node.parentNode;
      }
      return null;
    },

    get id() { return this.getAttribute('id') ?? ''; },
    set id(v) { this.setAttribute('id', v); },
    get href() { return this.getAttribute('href') ?? ''; },
    set href(v) { this.setAttribute('href', v); },

    appendChild(child) { return _appendChild(this, child); },
    insertBefore(child, ref) { return _insertBefore(this, child, ref); },
    removeChild(child) { return _removeChild(this, child); },
    cloneNode(deep = false) { return _cloneNode(this, deep); },

    focus() { _focusElement(this); },
    blur() { _blurElement(this); },
    click() {},
    scrollIntoView() {},
  };
  _applySiblingGetters(el, () => el.parentNode);
  _attachElementProps(el);
  return el;
}

function createTextNode(text) {
  const node = {
    nodeType: TEXT_NODE,
    nodeValue: text,
    get data() { return this.nodeValue; },
    set data(v) { this.nodeValue = v; },
    childNodes: [],
    parentNode: null,
    cloneNode() { return createTextNode(this.nodeValue); },
  };
  _applySiblingGetters(node, () => node.parentNode);
  return node;
}

function createComment(data) {
  const node = {
    nodeType: COMMENT_NODE,
    data,
    get nodeValue() { return this.data; },
    set nodeValue(v) { this.data = v; },
    childNodes: [],
    parentNode: null,
    cloneNode() { return createComment(this.data); },
  };
  _applySiblingGetters(node, () => node.parentNode);
  return node;
}

function createDocumentFragment() {
  const frag = {
    nodeType: DOCUMENT_FRAGMENT_NODE,
    childNodes: [],
    parentNode: null,
    appendChild(child) { return _appendChild(this, child); },
    insertBefore(child, ref) { return _insertBefore(this, child, ref); },
    removeChild(child) { return _removeChild(this, child); },
    cloneNode(deep = false) { return _cloneNode(this, deep); },
  };
  _applySiblingGetters(frag, () => frag.parentNode);
  return frag;
}

/**
 * Construct a real-DOM-shaped Event with capture/target/bubble metadata.
 * @internal
 * @param {string} type
 * @param {{ bubbles?: boolean, cancelable?: boolean, composed?: boolean }} [init]
 * @returns {object}
 */
function _makeEvent(type, init) {
  init = init || {};
  const ev = {
    type: String(type),
    bubbles: Boolean(init.bubbles),
    cancelable: Boolean(init.cancelable),
    composed: Boolean(init.composed),
    defaultPrevented: false,
    target: null,
    currentTarget: null,
    eventPhase: 0,
    _stopPropagation: false,
    _stopImmediate: false,
    preventDefault() { if (ev.cancelable) ev.defaultPrevented = true; },
    stopPropagation() { ev._stopPropagation = true; },
    stopImmediatePropagation() { ev._stopPropagation = true; ev._stopImmediate = true; },
  };
  return ev;
}

/**
 * @param {string} type
 * @param {object} [init]
 * @returns {object}
 */
export function Event(type, init) {
  return _makeEvent(type, init);
}

/**
 * @param {string} type
 * @param {{ detail?: unknown } & object} [init]
 * @returns {object}
 */
export function CustomEvent(type, init) {
  const ev = _makeEvent(type, init);
  ev.detail = init && "detail" in init ? init.detail : null;
  return ev;
}

/**
 * @param {string} type
 * @param {{ key?: string, code?: string, altKey?: boolean, ctrlKey?: boolean, metaKey?: boolean, shiftKey?: boolean, repeat?: boolean } & object} [init]
 * @returns {object}
 */
export function KeyboardEvent(type, init) {
  const ev = _makeEvent(type, init);
  init = init || {};
  ev.key = init.key ?? "";
  ev.code = init.code ?? "";
  ev.altKey = Boolean(init.altKey);
  ev.ctrlKey = Boolean(init.ctrlKey);
  ev.metaKey = Boolean(init.metaKey);
  ev.shiftKey = Boolean(init.shiftKey);
  ev.repeat = Boolean(init.repeat);
  return ev;
}

/**
 * @param {string} type
 * @param {{ clientX?: number, clientY?: number, screenX?: number, screenY?: number, button?: number, buttons?: number, altKey?: boolean, ctrlKey?: boolean, metaKey?: boolean, shiftKey?: boolean } & object} [init]
 * @returns {object}
 */
export function MouseEvent(type, init) {
  const ev = _makeEvent(type, init);
  init = init || {};
  ev.clientX = init.clientX ?? 0;
  ev.clientY = init.clientY ?? 0;
  ev.screenX = init.screenX ?? 0;
  ev.screenY = init.screenY ?? 0;
  ev.button = init.button ?? 0;
  ev.buttons = init.buttons ?? 0;
  ev.altKey = Boolean(init.altKey);
  ev.ctrlKey = Boolean(init.ctrlKey);
  ev.metaKey = Boolean(init.metaKey);
  ev.shiftKey = Boolean(init.shiftKey);
  return ev;
}

function _makeEventTarget() {
  const _listeners = new Map();
  return {
    _listeners,
    addEventListener(event, handler, options) {
      if (!_listeners.has(event)) _listeners.set(event, []);
      _listeners.get(event).push({
        handler,
        once: options?.once ?? false,
        // Record the capture flag (boolean or options form) so a bubbling
        // _dispatchEvent fires these listeners in the right phase; without
        // it, `entry.capture !== capture` skipped every document/window
        // listener for events dispatched on descendants.
        capture: options === true || (options?.capture ?? false),
      });
    },
    removeEventListener(event, handler) {
      if (!_listeners.has(event)) return;
      _listeners.set(event, _listeners.get(event).filter(e => e.handler !== handler));
    },
    dispatchEvent(event) {
      if (event.defaultPrevented == null) event.defaultPrevented = false;
      const origPreventDefault = event.preventDefault;
      event.preventDefault = () => {
        event.defaultPrevented = true;
        if (origPreventDefault) origPreventDefault.call(event);
      };
      const list = _listeners.get(event.type);
      if (!list) return;
      const toRemove = [];
      for (const entry of [...list]) {
        entry.handler(event);
        if (entry.once) toRemove.push(entry.handler);
      }
      for (const h of toRemove) this.removeEventListener(event.type, h);
    },
  };
}

function createElementNS(ns, tagName) {
  const el = createElement(tagName);
  el.namespaceURI = ns;
  return el;
}

/**
 * Update `document._activeElement` to `el`, dispatching a `blur` on the
 * previously focused element and a `focus` on `el`.
 * @internal
 * @param {object} el
 * @returns {void}
 */
function _focusElement(el) {
  const prev = document._activeElement;
  if (prev === el) return;
  if (prev) {
    document._activeElement = null;
    _dispatchEvent(prev, _makeEvent("blur", {}));
  }
  document._activeElement = el;
  _dispatchEvent(el, _makeEvent("focus", {}));
}

/**
 * Clear `document._activeElement` when `el` currently holds focus and
 * dispatch a `blur` event on it.
 * @internal
 * @param {object} el
 * @returns {void}
 */
function _blurElement(el) {
  if (document._activeElement !== el) return;
  document._activeElement = null;
  _dispatchEvent(el, _makeEvent("blur", {}));
}

let _documentElement = createElement("html");
let _head = createElement("head");
let _body = createElement("body");
_appendChild(_documentElement, _head);
_appendChild(_documentElement, _body);

/**
 * Walk descendants of `_documentElement` looking for the first element with
 * matching id attribute.
 * @internal
 * @param {string} id
 * @returns {object|null}
 */
function _getElementById(id) {
  let found = null;
  _walkDescendants(_documentElement, node => {
    if (!found && node.nodeType === ELEMENT_NODE && node.getAttribute("id") === id) {
      found = node;
    }
  });
  return found;
}

export const document = Object.assign(
  {
    createElement,
    createElementNS,
    createTextNode,
    createComment,
    createDocumentFragment,
    childNodes: [_documentElement],
    _activeElement: null,
    _title: "",
    get documentElement() { return _documentElement; },
    set documentElement(v) { _documentElement = v; },
    get head() { return _head; },
    set head(v) { _head = v; },
    get body() { return _body; },
    set body(v) { _body = v; },
    get activeElement() { return this._activeElement; },
    get title() { return this._title; },
    set title(v) { this._title = v == null ? "" : String(v); },
    getElementById(id) { return _getElementById(String(id)); },
    appendChild(child) { return _appendChild(this, child); },
    querySelector(selector) {
      const parsed = _parseComplexSelector(selector);
      let found = null;
      _walkDescendants(this, node => {
        if (!found && node.nodeType === ELEMENT_NODE && _matchComplexSelector(node, parsed, this)) found = node;
      });
      return found;
    },
    querySelectorAll(selector) {
      const parsed = _parseComplexSelector(selector);
      const results = [];
      _walkDescendants(this, node => {
        if (node.nodeType === ELEMENT_NODE && _matchComplexSelector(node, parsed, this)) results.push(node);
      });
      return results;
    },
  },
  _makeEventTarget(),
);
_documentElement.parentNode = document;

const _windowEventTarget = _makeEventTarget();

const _location = {
  origin: 'http://localhost',
  pathname: '/',
  search: '',
  hash: '',
  get href() { return this.origin + this.pathname + this.search + this.hash; },
  _set(input) {
    const hashIdx = input.indexOf('#');
    const noHash = hashIdx >= 0 ? input.slice(0, hashIdx) : input;
    this.hash = hashIdx >= 0 ? input.slice(hashIdx) : '';
    const qIdx = noHash.indexOf('?');
    if (qIdx >= 0) {
      this.pathname = noHash.slice(0, qIdx);
      this.search = noHash.slice(qIdx);
    } else {
      this.pathname = noHash;
      this.search = '';
    }
  },
};

const _history = {
  _entries: [{ state: null, url: '/' }],
  _index: 0,
  get length() { return this._entries.length; },
  pushState(state, _title, url) {
    this._entries.splice(this._index + 1);
    this._entries.push({ state, url });
    this._index = this._entries.length - 1;
    _location._set(url);
  },
  replaceState(state, _title, url) {
    this._entries[this._index] = { state, url };
    _location._set(url);
  },
  back() {
    if (this._index > 0) {
      this._index--;
      _location._set(this._entries[this._index].url);
      exports_window.dispatchEvent({ type: 'popstate', state: this._entries[this._index].state });
    }
  },
  forward() {
    if (this._index < this._entries.length - 1) {
      this._index++;
      _location._set(this._entries[this._index].url);
      exports_window.dispatchEvent({ type: 'popstate', state: this._entries[this._index].state });
    }
  },
};

export const window = Object.assign(_windowEventTarget, {
  location: _location,
  history: _history,
});

/**
 * Build an in-memory Storage-shaped object (`getItem`, `setItem`, `removeItem`,
 * `clear`, `key`, `length`). Backed by a plain object plus an insertion-order
 * array instead of a `Map` so Boa's MapLock GC bug stays out of reach.
 * @internal
 * @returns {object}
 */
function _makeStorage() {
  const data = Object.create(null);
  const order = [];
  return {
    getItem(k) {
      const key = String(k);
      return key in data ? data[key] : null;
    },
    setItem(k, v) {
      const key = String(k);
      if (!(key in data)) order.push(key);
      data[key] = String(v);
    },
    removeItem(k) {
      const key = String(k);
      if (key in data) {
        delete data[key];
        const idx = order.indexOf(key);
        if (idx >= 0) order.splice(idx, 1);
      }
    },
    clear() {
      for (const k of order) delete data[k];
      order.length = 0;
    },
    key(i) { return order[i] ?? null; },
    get length() { return order.length; },
  };
}

export const localStorage = _makeStorage();
export const sessionStorage = _makeStorage();

/**
 * Install `setTimeout` / `clearTimeout` / `setInterval` / `clearInterval` /
 * `requestAnimationFrame` / `cancelAnimationFrame` on `globalThis`, backed by
 * a single JS-side registry whose microtask wrapper drains pending ids from a
 * queue. `ms` is ignored — all pending timers fire in FIFO order on the next
 * job-queue drain (typically the next `await`). Returns the registry so tests
 * can introspect.
 *
 * Implemented entirely in JS so the Boa host does not need any unsafe closure
 * captures of `JsValue` — every cross-call piece of state lives on the
 * registry object that lives on `globalThis`.
 * @internal
 * @returns {void}
 */
function _installTimerHost() {
  if (typeof globalThis.setTimeout === "function") return;
  const state = {
    next: 0,
    cb: Object.create(null),
    cancelled: Object.create(null),
    intervals: Object.create(null),
    raf: Object.create(null),
    queue: [],
    rafCounter: 0,
  };
  function fire() {
    if (state.queue.length === 0) return;
    const id = state.queue.shift();
    if (state.cancelled[id]) {
      delete state.cb[id];
      delete state.cancelled[id];
      delete state.intervals[id];
      delete state.raf[id];
      return;
    }
    const cb = state.cb[id];
    const isInterval = Boolean(state.intervals[id]);
    const isRaf = Boolean(state.raf[id]);
    if (!isInterval) delete state.cb[id];
    if (typeof cb !== "function") return;
    if (isRaf) {
      delete state.raf[id];
      state.rafCounter += 16;
      cb(state.rafCounter);
    } else {
      cb();
    }
    if (isInterval && !state.cancelled[id]) {
      state.queue.push(id);
      Promise.resolve().then(fire);
    }
  }
  function schedule(cb, isInterval, isRaf) {
    const id = ++state.next;
    state.cb[id] = cb;
    if (isInterval) state.intervals[id] = true;
    if (isRaf) state.raf[id] = true;
    state.queue.push(id);
    Promise.resolve().then(fire);
    return id;
  }
  function cancel(id) {
    if (id != null) state.cancelled[id] = true;
  }
  function clearAll() {
    for (let i = 1; i <= state.next; i++) state.cancelled[i] = true;
  }
  globalThis.__zero_timers__ = state;
  globalThis.setTimeout = (cb, _ms) => schedule(cb, false, false);
  globalThis.setInterval = (cb, _ms) => schedule(cb, true, false);
  globalThis.requestAnimationFrame = cb => schedule(cb, false, true);
  globalThis.clearTimeout = cancel;
  globalThis.clearInterval = cancel;
  globalThis.cancelAnimationFrame = cancel;
  globalThis.__clearAllTimers__ = clearAll;
}

/**
 * Build a MediaQueryList-shaped object. The shim's `.matches` is always
 * `false`; tests that want a specific outcome reassign `window.matchMedia`.
 * @internal
 * @param {string} query
 * @returns {object}
 */
function _makeMediaQueryList(query) {
  const target = _makeEventTarget();
  const mql = Object.assign(target, {
    media: String(query),
    matches: false,
    onchange: null,
    addListener(fn) { target.addEventListener("change", fn); },
    removeListener(fn) { target.removeEventListener("change", fn); },
  });
  const baseDispatch = target.dispatchEvent.bind(target);
  mql.dispatchEvent = function(event) {
    baseDispatch(event);
    if (typeof mql.onchange === "function" && event.type === "change") {
      mql.onchange.call(mql, event);
    }
  };
  return mql;
}

/**
 * @internal
 * @returns {object}
 */
function _makeNavigator() {
  return {
    userAgent: "zero-test-shim/1.0",
    language: "en-US",
    languages: ["en-US"],
    onLine: true,
    platform: "",
  };
}

/**
 * Build a {@link Crypto}-shaped object. Not cryptographically strong — uses
 * `Math.random()` to fill bytes. Use case is store IDs, not secrets.
 * @internal
 * @returns {object}
 */
function _makeCrypto() {
  function randomUUID() {
    const bytes = new Array(16);
    for (let i = 0; i < 16; i++) bytes[i] = Math.floor(Math.random() * 256);
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    const hex = bytes.map(b => b.toString(16).padStart(2, "0")).join("");
    return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`;
  }
  function getRandomValues(arr) {
    for (let i = 0; i < arr.length; i++) {
      arr[i] = Math.floor(Math.random() * 0x100000000);
    }
    return arr;
  }
  return { randomUUID, getRandomValues };
}

/**
 * Build an Observer constructor (Intersection/Resize/Mutation). The shim's
 * observers never fire automatically — tests trigger callbacks manually.
 * @internal
 * @param {boolean} withTakeRecords
 * @returns {Function}
 */
function _makeObserver(withTakeRecords) {
  return function Observer(callback) {
    this.callback = callback;
    this.observations = [];
    this.observe = (target, options) => {
      this.observations.push({ target, options });
    };
    this.unobserve = (target) => {
      this.observations = this.observations.filter(o => o.target !== target);
    };
    this.disconnect = () => { this.observations.length = 0; };
    if (withTakeRecords) this.takeRecords = () => [];
  };
}

/**
 * @internal
 * @returns {object}
 */
function _makeComputedStyle() {
  return {
    getPropertyValue() { return ""; },
    setProperty() { throw new Error("getComputedStyle result is read-only"); },
    length: 0,
  };
}

window.matchMedia = function(query) { return _makeMediaQueryList(query); };
window.navigator = _makeNavigator();
window.getComputedStyle = function() { return _makeComputedStyle(); };

const _IntersectionObserver = _makeObserver(false);
const _ResizeObserver = _makeObserver(false);
const _MutationObserver = _makeObserver(true);

Object.defineProperty(globalThis, "navigator", {
  value: window.navigator,
  writable: true,
  configurable: true,
});
Object.defineProperty(globalThis, "crypto", {
  value: _makeCrypto(),
  writable: true,
  configurable: true,
});
window.crypto = globalThis.crypto;
Object.defineProperty(globalThis, "IntersectionObserver", {
  value: _IntersectionObserver,
  writable: true,
  configurable: true,
});
Object.defineProperty(globalThis, "ResizeObserver", {
  value: _ResizeObserver,
  writable: true,
  configurable: true,
});
Object.defineProperty(globalThis, "MutationObserver", {
  value: _MutationObserver,
  writable: true,
  configurable: true,
});

// Forward-reference alias so _history.back/forward can reference window after construction.
const exports_window = window;

if (typeof globalThis.document === 'undefined') {
  globalThis.document = document;
}

if (typeof globalThis.window === 'undefined') {
  globalThis.window = window;
}

if (typeof globalThis.Event === 'undefined') globalThis.Event = Event;
if (typeof globalThis.CustomEvent === 'undefined') globalThis.CustomEvent = CustomEvent;
if (typeof globalThis.KeyboardEvent === 'undefined') globalThis.KeyboardEvent = KeyboardEvent;
if (typeof globalThis.MouseEvent === 'undefined') globalThis.MouseEvent = MouseEvent;

Object.defineProperty(globalThis, 'localStorage', { value: localStorage, writable: true, configurable: true });
Object.defineProperty(globalThis, 'sessionStorage', { value: sessionStorage, writable: true, configurable: true });
window.localStorage = localStorage;
window.sessionStorage = sessionStorage;

_installTimerHost();
