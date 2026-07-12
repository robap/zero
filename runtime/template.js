import { effect, _createScope } from './reactivity.js';
// `document` is read from globalThis at call time (set by dom-shim in tests,
// real DOM in production).

/**
 * @typedef {{ type: 'attr', path: number[], name: string, statics: string[] }} AttrPart
 *   `statics.length === valueCount + 1`. For `class=${x}` the statics array
 *   is `['', '']` (one value); for `class="a ${x} b ${y} c"` it is
 *   `['a ', ' b ', ' c']` (two values). The commit loop advances the value
 *   cursor by `statics.length - 1` for each attr part.
 */

/**
 * @typedef {{ type: 'event', path: number[], event: string, modifiers: string[] }} EventPart
 */

/**
 * @typedef {{ type: 'ref', path: number[] }} RefPart
 */

/**
 * @typedef {{ type: 'node', path: number[] }} NodePart
 */

/**
 * @typedef {AttrPart | EventPart | RefPart | NodePart} Part
 */

/**
 * @typedef {{ fragment: DocumentFragment, parts: Part[] }} Template
 */

/**
 * @typedef {{ _template: Template, _values: any[] }} TemplateResult
 */

/**
 * @template T
 * @typedef {{ _isEach: true, signal: { readonly val: T[] }, renderFn: (item: T, index: number) => TemplateResult }} EachMarker
 */

const _templateCache = new WeakMap();

// Parser states
const TEXT = 'TEXT';
const TAG_OPEN = 'TAG_OPEN';
const TAG_NAME = 'TAG_NAME';
const IN_TAG = 'IN_TAG';
const ATTR_NAME = 'ATTR_NAME';
const AFTER_ATTR_NAME = 'AFTER_ATTR_NAME';
const ATTR_VALUE_UNQUOTED = 'ATTR_VALUE_UNQUOTED';
const ATTR_VALUE_DQ = 'ATTR_VALUE_DQ';
const ATTR_VALUE_SQ = 'ATTR_VALUE_SQ';
const CLOSING_TAG = 'CLOSING_TAG';

/**
 * @param {TemplateStringsArray} strings
 * @returns {Template}
 */
const SVG_NS = 'http://www.w3.org/2000/svg';

function _parseTemplate(strings) {
  const frag = document.createDocumentFragment();
  const parts = [];

  let state = TEXT;
  let parent = frag;
  const elementStack = [];
  const parentPath = [];
  let currentAttrName = '';
  let currentAttrValue = '';
  // Whether an `=` has been seen for the current attribute. Distinguishes
  // `readonly value` (bare boolean, then a new attribute) from `name=value`
  // (an unquoted value) once we are in AFTER_ATTR_NAME.
  let sawEquals = false;
  /** @type {string[] | null} */
  let attrStatics = null;
  let attrHasPlaceholders = false;
  let currentTagName = '';
  let currentCloseTagName = '';
  let currentTextData = '';
  // Count of currently-open SVG-namespaced ancestors. When > 0, newly
  // created elements use createElementNS so the browser renders them as
  // SVG (HTML-namespaced <svg>/<circle>/<path>/... are not painted).
  let svgDepth = 0;

  function createEl(tagName) {
    if (svgDepth > 0 || tagName.toLowerCase() === 'svg') {
      return document.createElementNS(SVG_NS, tagName);
    }
    return document.createElement(tagName);
  }

  function flushText() {
    if (currentTextData) {
      parent.appendChild(document.createTextNode(currentTextData));
      currentTextData = '';
    }
  }

  /**
   * Close out the current attribute. If `attrHasPlaceholders`, push the
   * trailing static fragment and emit a multi-placeholder `attr` part.
   * Otherwise, set the attribute statically on the parent element.
   * @internal
   * @returns {void}
   */
  function flushAttr() {
    if (!currentAttrName) return;
    if (attrHasPlaceholders) {
      attrStatics.push(currentAttrValue);
      parts.push({
        type: 'attr',
        path: [...parentPath],
        name: currentAttrName,
        statics: attrStatics,
      });
    } else {
      parent.setAttribute(currentAttrName, currentAttrValue);
    }
    currentAttrName = '';
    currentAttrValue = '';
    attrStatics = null;
    attrHasPlaceholders = false;
  }

  function childPath(childIndex) {
    return [...parentPath, childIndex];
  }

  for (let si = 0; si < strings.length; si++) {
    const s = strings[si];

    for (let ci = 0; ci < s.length; ci++) {
      const ch = s[ci];
      const next = s[ci + 1];

      switch (state) {
        case TEXT:
          if (ch === '<') {
            flushText();
            if (next === '/') {
              state = CLOSING_TAG;
              currentCloseTagName = '';
              ci++; // skip '/'
            } else {
              state = TAG_OPEN;
              currentTagName = '';
            }
          } else {
            currentTextData += ch;
          }
          break;

        case TAG_OPEN:
          if (/[a-zA-Z]/.test(ch)) {
            state = TAG_NAME;
            currentTagName = ch;
          } else {
            throw new Error(`html: unexpected char '${ch}' after '<'`);
          }
          break;

        case TAG_NAME:
          if (/[a-zA-Z0-9\-]/.test(ch)) {
            currentTagName += ch;
          } else if (ch === ' ' || ch === '\t' || ch === '\n' || ch === '\r') {
            const el = createEl(currentTagName);
            if (el.namespaceURI === SVG_NS) svgDepth++;
            parent.appendChild(el);
            elementStack.push({ el, pathIdx: parent.childNodes.length - 1, svg: el.namespaceURI === SVG_NS });
            parentPath.push(parent.childNodes.length - 1);
            parent = el;
            state = IN_TAG;
          } else if (ch === '>') {
            const el = createEl(currentTagName);
            if (el.namespaceURI === SVG_NS) svgDepth++;
            parent.appendChild(el);
            elementStack.push({ el, pathIdx: parent.childNodes.length - 1, svg: el.namespaceURI === SVG_NS });
            parentPath.push(parent.childNodes.length - 1);
            parent = el;
            state = TEXT;
          } else if (ch === '/' && next === '>') {
            const el = createEl(currentTagName);
            parent.appendChild(el);
            // self-closing: do not push to element stack; svgDepth unchanged.
            ci++; // skip '>'
            state = TEXT;
          } else {
            throw new Error(`html: unexpected char '${ch}' in tag name`);
          }
          break;

        case IN_TAG:
          if (ch === '>') {
            state = TEXT;
          } else if (ch === '/' && next === '>') {
            // self-closing for element already pushed
            elementStack.pop();
            parentPath.pop();
            parent = elementStack.length > 0 ? elementStack[elementStack.length - 1].el : frag;
            ci++;
            state = TEXT;
          } else if (ch !== ' ' && ch !== '\t' && ch !== '\n' && ch !== '\r') {
            state = ATTR_NAME;
            currentAttrName = ch;
            sawEquals = false;
          }
          break;

        case ATTR_NAME:
          if (ch === '=') {
            state = AFTER_ATTR_NAME;
            sawEquals = true;
          } else if (ch === ' ' || ch === '\t' || ch === '\n' || ch === '\r') {
            // Boolean attribute followed by whitespace — may still get `=value`.
            state = AFTER_ATTR_NAME;
          } else if (ch === '>') {
            // Boolean attribute terminating the tag, e.g. `<input disabled>`.
            flushAttr();
            state = TEXT;
          } else {
            currentAttrName += ch;
          }
          break;

        case AFTER_ATTR_NAME:
          if (ch === '"') {
            state = ATTR_VALUE_DQ;
          } else if (ch === "'") {
            state = ATTR_VALUE_SQ;
          } else if (ch === '>') {
            // Boolean attribute terminating the tag, e.g. `<input disabled >`.
            flushAttr();
            state = TEXT;
          } else if (ch === '=') {
            sawEquals = true;
          } else if (ch === ' ' || ch === '\t' || ch === '\n' || ch === '\r') {
            // still in whitespace after attr name
          } else if (sawEquals) {
            // Unquoted attribute value — capture the first character.
            state = ATTR_VALUE_UNQUOTED;
            currentAttrValue = ch;
          } else {
            // No `=` was seen: the current attribute is a bare boolean and
            // this char begins the next attribute (or a tag terminator like
            // `/>`). Flush the boolean and re-process this char in IN_TAG.
            flushAttr();
            state = IN_TAG;
            ci--;
          }
          break;

        case ATTR_VALUE_DQ:
          if (ch === '"') {
            flushAttr();
            state = IN_TAG;
          } else {
            currentAttrValue += ch;
          }
          break;

        case ATTR_VALUE_SQ:
          if (ch === "'") {
            flushAttr();
            state = IN_TAG;
          } else {
            currentAttrValue += ch;
          }
          break;

        case ATTR_VALUE_UNQUOTED:
          if (ch === ' ' || ch === '\t' || ch === '\n' || ch === '\r') {
            flushAttr();
            state = IN_TAG;
          } else if (ch === '>') {
            flushAttr();
            state = TEXT;
          } else {
            currentAttrValue += ch;
          }
          break;

        case CLOSING_TAG:
          if (ch === '>') {
            const popped = elementStack.pop();
            if (popped && popped.svg) svgDepth--;
            parentPath.pop();
            parent = elementStack.length > 0 ? elementStack[elementStack.length - 1].el : frag;
            currentCloseTagName = '';
            state = TEXT;
          } else {
            currentCloseTagName += ch;
          }
          break;
      }
    }

    // Between strings[si] and strings[si+1], a placeholder value sits here.
    // Record a Part based on current parser state.
    if (si < strings.length - 1) {
      flushText();
      switch (state) {
        case TEXT: {
          const anchor = document.createComment('');
          parent.appendChild(anchor);
          parts.push({ type: 'node', path: childPath(parent.childNodes.length - 1) });
          break;
        }
        case AFTER_ATTR_NAME:
        case ATTR_VALUE_DQ:
        case ATTR_VALUE_SQ:
        case ATTR_VALUE_UNQUOTED: {
          const path = [...parentPath];
          if (currentAttrName.startsWith('@')) {
            const [eventPart, ...modifiers] = currentAttrName.slice(1).split('.');
            parts.push({ type: 'event', path, event: eventPart, modifiers });
            currentAttrName = '';
            currentAttrValue = '';
            if (state === AFTER_ATTR_NAME) state = IN_TAG;
          } else if (currentAttrName === 'ref') {
            parts.push({ type: 'ref', path });
            currentAttrName = '';
            currentAttrValue = '';
            if (state === AFTER_ATTR_NAME) state = IN_TAG;
          } else {
            // attr — accumulate static fragment, defer emit until value closes.
            if (attrStatics === null) attrStatics = [];
            attrStatics.push(currentAttrValue);
            currentAttrValue = '';
            attrHasPlaceholders = true;
            // AFTER_ATTR_NAME means unquoted `name=${x}` — switch into
            // ATTR_VALUE_UNQUOTED so the trailing terminator (whitespace
            // or `>`) closes the value via flushAttr().
            if (state === AFTER_ATTR_NAME) state = ATTR_VALUE_UNQUOTED;
          }
          break;
        }
        default:
          throw new Error(`html: placeholder in unsupported position (state: ${state})`);
      }
    }
  }

  flushText();

  return { fragment: frag, parts };
}

/**
 * @param {TemplateStringsArray} strings
 * @param {...any} values
 * @returns {TemplateResult}
 */
export function html(strings, ...values) {
  let template = _templateCache.get(strings);
  if (!template) {
    template = _parseTemplate(strings);
    _templateCache.set(strings, template);
  }
  return { _template: template, _values: values };
}

// Key modifier map for event handling
const KEY_MODIFIERS = {
  enter: 'Enter',
  escape: 'Escape',
  space: ' ',
  tab: 'Tab',
  up: 'ArrowUp',
  down: 'ArrowDown',
  left: 'ArrowLeft',
  right: 'ArrowRight',
};

function _isReactive(v) {
  if (v == null || typeof v !== 'object') return false;
  const desc = Object.getOwnPropertyDescriptor(v, 'val');
  return !!desc && typeof desc.get === 'function';
}

function _isTemplateResult(v) {
  return v != null && typeof v === 'object' && v._template != null && Array.isArray(v._values);
}

function _walkPath(root, path) {
  let node = root;
  for (const i of path) node = node.childNodes[i];
  return node;
}

/**
 * If `(el, name)` is a live form property (`value` on input/textarea/select,
 * `checked` on input, `selected` on option), set the DOM property instead of
 * the content attribute and return `true`; otherwise return `false` so the
 * caller falls back to attribute handling. The content attribute is only the
 * *default* — browsers track the shown/checked/selected state on the property,
 * so a late `setAttribute` is ignored for what the user sees.
 * @internal
 * @param {Element} el
 * @param {string} name
 * @param {unknown} v
 * @returns {boolean}
 */
function _applyLiveProp(el, name, v) {
  const tag = el.tagName;
  if (name === 'value' && (tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT')) {
    const next = v == null ? '' : String(v);
    // Guard: assigning the value the user already typed would move the caret
    // to the end on every keystroke of a controlled input.
    if (el.value !== next) el.value = next;
    return true;
  }
  if (name === 'checked' && tag === 'INPUT') { el.checked = !!v && v !== 'false'; return true; }
  if (name === 'selected' && tag === 'OPTION') { el.selected = !!v && v !== 'false'; return true; }
  return false;
}

function _applyAttr(el, name, v) {
  if (_applyLiveProp(el, name, v)) return;
  if (v === false || v == null) el.removeAttribute(name);
  else if (v === true) el.setAttribute(name, '');
  else el.setAttribute(name, String(v));
}

/**
 * @internal
 * @param {Element} el
 * @param {string} name
 * @param {string[]} statics
 * @param {unknown[]} values
 * @returns {void}
 */
function _commitAttr(el, name, statics, values) {
  if (statics.length === 2 && statics[0] === '' && statics[1] === '') {
    _commitAttrSingle(el, name, values[0]);
    return;
  }
  _commitAttrJoined(el, name, statics, values);
}

/**
 * @internal
 * @param {Element} el
 * @param {string} name
 * @param {unknown} value
 * @returns {void}
 */
function _commitAttrSingle(el, name, value) {
  if (_isReactive(value)) {
    effect(() => _applyAttr(el, name, value.val));
  } else if (typeof value === 'function') {
    effect(() => _applyAttr(el, name, value()));
  } else {
    _applyAttr(el, name, value);
  }
}

/**
 * @internal
 * @param {Element} el
 * @param {string} name
 * @param {string[]} statics
 * @param {unknown[]} values
 * @returns {void}
 */
function _commitAttrJoined(el, name, statics, values) {
  const anyReactive = values.some(v => _isReactive(v) || typeof v === 'function');
  if (anyReactive) {
    effect(() => _setJoinedAttr(el, name, statics, values));
  } else {
    _setJoinedAttr(el, name, statics, values);
  }
}

/**
 * @internal
 * @param {Element} el
 * @param {string} name
 * @param {string[]} statics
 * @param {unknown[]} values
 * @returns {void}
 */
function _setJoinedAttr(el, name, statics, values) {
  let out = statics[0];
  for (let i = 0; i < values.length; i++) {
    out += _coerceConcatValue(values[i]) + statics[i + 1];
  }
  if (_applyLiveProp(el, name, out)) return;
  el.setAttribute(name, out);
}

/**
 * @internal
 * @param {unknown} v
 * @returns {string}
 */
function _coerceConcatValue(v) {
  if (v == null) return '';
  if (_isReactive(v)) return _coerceConcatValue(v.val);
  if (typeof v === 'function') return _coerceConcatValue(v());
  return String(v);
}

function _nextSiblingAfter(anchor, state) {
  if (state.currentNodes.length === 0) return anchor.nextSibling;
  const last = state.currentNodes[state.currentNodes.length - 1];
  return last.nextSibling;
}

function _clearNodeContent(state) {
  for (const node of state.currentNodes) {
    if (node.parentNode) node.parentNode.removeChild(node);
  }
  state.currentNodes.length = 0;
}

function _disposeItemScopes(state) {
  if (!state.itemScopes) return;
  for (const s of state.itemScopes) s.dispose();
  state.itemScopes.length = 0;
}

function _clearNodeSlot(anchor, state) {
  _disposeItemScopes(state);
  _clearNodeContent(state);
}

function _appendNodeItem(anchor, value, state) {
  if (value == null) return;

  if (_isTemplateResult(value)) {
    const frag = document.createDocumentFragment();
    commit(value, frag);
    while (frag.childNodes.length > 0) {
      const node = frag.childNodes[0];
      anchor.parentNode.insertBefore(node, _nextSiblingAfter(anchor, state));
      state.currentNodes.push(node);
    }
    return;
  }

  const text = document.createTextNode(String(value));
  anchor.parentNode.insertBefore(text, _nextSiblingAfter(anchor, state));
  state.currentNodes.push(text);
}

function _applyNodeValueLeaf(anchor, value, state) {
  _clearNodeContent(state);

  if (value == null) return;

  if (Array.isArray(value)) {
    for (const item of value) _appendNodeItem(anchor, item, state);
    return;
  }

  _appendNodeItem(anchor, value, state);
}

function _commitEach(anchor, eachMarker, state) {
  if (typeof eachMarker.keyFn === 'function') {
    _commitEachKeyed(anchor, eachMarker, state);
    return;
  }
  const { signal: arrSig, renderFn } = eachMarker;
  state.itemScopes = state.itemScopes || [];

  effect(() => {
    _disposeItemScopes(state);
    _clearNodeContent(state);

    const items = arrSig.val;
    if (!Array.isArray(items)) return;

    for (let i = 0; i < items.length; i++) {
      const scope = _createScope();
      state.itemScopes.push(scope);
      scope.run(() => {
        const tr = renderFn(items[i], i);
        const frag = document.createDocumentFragment();
        commit(tr, frag);
        while (frag.childNodes.length > 0) {
          const node = frag.childNodes[0];
          anchor.parentNode.insertBefore(node, _nextSiblingAfter(anchor, state));
          state.currentNodes.push(node);
        }
      });
    }
  });
}

function _commitEachKeyed(anchor, eachMarker, state) {
  const { signal: arrSig, renderFn, keyFn } = eachMarker;
  state.itemsByKey = state.itemsByKey || Object.create(null);

  effect(() => {
    const items = arrSig.val;
    if (!Array.isArray(items)) {
      for (const k in state.itemsByKey) state.itemsByKey[k].scope.dispose();
      state.itemsByKey = Object.create(null);
      _clearNodeContent(state);
      return;
    }

    const newKeys = new Array(items.length);
    const seen = Object.create(null);
    for (let i = 0; i < items.length; i++) {
      const k = String(keyFn(items[i], i));
      if (seen[k]) {
        throw new Error(`each: duplicate key '${k}' in row ${i}`);
      }
      seen[k] = true;
      newKeys[i] = k;
    }

    const oldMap = state.itemsByKey;
    const newMap = Object.create(null);
    const parent = anchor.parentNode;

    for (const k in oldMap) {
      if (!seen[k]) {
        const entry = oldMap[k];
        entry.scope.dispose();
        for (const node of entry.nodes) {
          if (node.parentNode) node.parentNode.removeChild(node);
        }
      }
    }

    const newCurrentNodes = [];
    let cursor = anchor.nextSibling;
    for (let i = 0; i < items.length; i++) {
      const k = newKeys[i];
      let entry = oldMap[k];
      if (entry == null) {
        const scope = _createScope();
        const nodes = [];
        scope.run(() => {
          const tr = renderFn(items[i], i);
          const frag = document.createDocumentFragment();
          commit(tr, frag);
          while (frag.childNodes.length > 0) {
            const node = frag.childNodes[0];
            parent.insertBefore(node, cursor);
            nodes.push(node);
            newCurrentNodes.push(node);
          }
        });
        entry = { scope, nodes };
      } else {
        for (const node of entry.nodes) {
          if (node !== cursor) {
            parent.insertBefore(node, cursor);
          } else {
            cursor = node.nextSibling;
          }
          newCurrentNodes.push(node);
        }
      }
      newMap[k] = entry;
      if (entry.nodes.length > 0) {
        cursor = entry.nodes[entry.nodes.length - 1].nextSibling;
      }
    }

    state.itemsByKey = newMap;
    state.currentNodes = newCurrentNodes;
  });
}

function _applyNodeValue(anchor, value, state) {
  _clearNodeSlot(anchor, state);

  if (value == null) return;

  if (value && value._isEach) {
    _commitEach(anchor, value, state);
    return;
  }

  if (_isReactive(value)) {
    effect(() => _applyNodeValueLeaf(anchor, value.val, state));
    return;
  }

  if (typeof value === 'function') {
    effect(() => _applyNodeValueLeaf(anchor, value(), state));
    return;
  }

  _applyNodeValueLeaf(anchor, value, state);
}

function _commitNode(anchor, value) {
  const state = { currentNodes: [] };
  _applyNodeValue(anchor, value, state);
  return state;
}

/**
 * Parse a timing modifier value (`throttle` / `debounce`) from the modifiers
 * list. Returns `0` when the modifier is absent, `100` for the bare form, or
 * the integer ms from a `:NNN` suffix. Throws on malformed suffixes
 * (`debounce:` / `debounce:abc` / `debounce:-5` / `debounce:1.5` /
 * `debounce:0`).
 * @internal
 * @param {string[]} modifiers
 * @param {'throttle' | 'debounce'} name
 * @returns {number}
 */
function _readTimingModifier(modifiers, name) {
  for (const m of modifiers) {
    if (m === name) return 100;
    if (m.startsWith(name + ':')) {
      const tail = m.slice(name.length + 1);
      if (!/^\d+$/.test(tail)) {
        throw new Error(`html: invalid modifier '${m}' — expected '${name}:<ms>' with positive integer`);
      }
      const n = Number(tail);
      if (n <= 0) {
        throw new Error(`html: invalid modifier '${m}' — interval must be > 0`);
      }
      return n;
    }
  }
  return 0;
}

function _wrapEventHandler(modifiers, handler) {
  const keyFilters = modifiers.filter(m => m in KEY_MODIFIERS);
  const hasPrevent = modifiers.includes('prevent');
  const hasStop = modifiers.includes('stop');
  const throttleMs = _readTimingModifier(modifiers, 'throttle');
  const debounceMs = _readTimingModifier(modifiers, 'debounce');

  let baseHandler = (e) => {
    if (keyFilters.length > 0 && !keyFilters.some(m => e.key === KEY_MODIFIERS[m])) return;
    if (hasPrevent) e.preventDefault?.();
    if (hasStop) e.stopPropagation?.();
    return handler(e);
  };

  if (throttleMs > 0) baseHandler = _throttle(baseHandler, throttleMs);
  if (debounceMs > 0) baseHandler = _debounce(baseHandler, debounceMs);

  return baseHandler;
}

function _throttle(fn, ms) {
  let last = 0;
  return (...args) => {
    const now = Date.now();
    if (now - last < ms) return;
    last = now;
    return fn(...args);
  };
}

function _debounce(fn, ms) {
  let timer;
  return (...args) => {
    clearTimeout(timer);
    timer = setTimeout(() => fn(...args), ms);
  };
}

function _commitEvent(el, eventName, modifiers, handler) {
  const wrapped = _wrapEventHandler(modifiers, handler);
  const options = modifiers.includes('once') ? { once: true } : undefined;
  el.addEventListener(eventName, wrapped, options);
  effect(() => () => el.removeEventListener(eventName, wrapped, options));
}

function _commitRef(el, refObj) {
  refObj.el = el;
  effect(() => () => { refObj.el = null; });
}

export function commit(templateResult, container) {
  const { _template, _values } = templateResult;
  const clone = _template.fragment.cloneNode(true);

  // Walk all paths before committing any part — insertions during commit
  // would otherwise shift subsequent path indices in the childNodes arrays.
  const targets = _template.parts.map(part => _walkPath(clone, part.path));

  let valueCursor = 0;
  for (let i = 0; i < _template.parts.length; i++) {
    const part = _template.parts[i];
    const target = targets[i];

    switch (part.type) {
      case 'attr': {
        const n = part.statics.length - 1;
        _commitAttr(target, part.name, part.statics, _values.slice(valueCursor, valueCursor + n));
        valueCursor += n;
        break;
      }
      case 'event': _commitEvent(target, part.event, part.modifiers, _values[valueCursor]); valueCursor++; break;
      case 'ref':   _commitRef(target, _values[valueCursor]); valueCursor++; break;
      case 'node':  _commitNode(target, _values[valueCursor]); valueCursor++; break;
    }
  }

  container.appendChild(clone);
}

/**
 * @template {Element} [T=Element]
 * @returns {{ el: T | null }}
 */
export function ref() {
  return { el: null };
}

/**
 * @template T
 * @param {{ readonly val: T[] }} sig
 * @param {(item: T, index: number) => TemplateResult} renderFn
 * @param {(item: T, index: number) => string | number} [keyFn]
 * @returns {EachMarker<T>}
 */
export function each(sig, renderFn, keyFn) {
  return { _isEach: true, signal: sig, renderFn, keyFn };
}
