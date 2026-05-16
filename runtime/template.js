import { effect, _createScope } from './reactivity.js';
// `document` is read from globalThis at call time (set by dom-shim in tests,
// real DOM in production).

/**
 * @typedef {{ fragment: DocumentFragment, parts: any[] }} Template
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

  function flushStaticAttr() {
    if (currentAttrName) {
      parent.setAttribute(currentAttrName, currentAttrValue);
      currentAttrName = '';
      currentAttrValue = '';
    }
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
          }
          break;

        case ATTR_NAME:
          if (ch === '=') {
            state = AFTER_ATTR_NAME;
          } else if (ch === ' ' || ch === '\t' || ch === '\n' || ch === '\r') {
            // Boolean attribute followed by whitespace — may still get `=value`.
            state = AFTER_ATTR_NAME;
          } else if (ch === '>') {
            // Boolean attribute terminating the tag, e.g. `<input disabled>`.
            flushStaticAttr();
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
            flushStaticAttr();
            state = TEXT;
          } else if (ch === '=') {
            // skip
          } else if (ch === ' ' || ch === '\t' || ch === '\n' || ch === '\r') {
            // still in whitespace after attr name
          } else {
            // Unquoted attribute value — capture the first character.
            state = ATTR_VALUE_UNQUOTED;
            currentAttrValue = ch;
          }
          break;

        case ATTR_VALUE_DQ:
          if (ch === '"') {
            flushStaticAttr();
            state = IN_TAG;
          } else {
            currentAttrValue += ch;
          }
          break;

        case ATTR_VALUE_SQ:
          if (ch === "'") {
            flushStaticAttr();
            state = IN_TAG;
          } else {
            currentAttrValue += ch;
          }
          break;

        case ATTR_VALUE_UNQUOTED:
          if (ch === ' ' || ch === '\t' || ch === '\n' || ch === '\r') {
            flushStaticAttr();
            state = IN_TAG;
          } else if (ch === '>') {
            flushStaticAttr();
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
          } else if (currentAttrName === 'ref') {
            parts.push({ type: 'ref', path });
          } else {
            parts.push({ type: 'attr', path, name: currentAttrName });
          }
          // The dynamic part owns this attribute now — clear static state so
          // the closing quote/space/`>` doesn't also setAttribute().
          currentAttrName = '';
          currentAttrValue = '';
          // Transition to IN_TAG so subsequent characters parse correctly
          if (state === AFTER_ATTR_NAME) state = IN_TAG;
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

function _applyAttr(el, name, v) {
  if (v === false || v == null) el.removeAttribute(name);
  else if (v === true) el.setAttribute(name, '');
  else el.setAttribute(name, String(v));
}

function _commitAttr(el, name, value) {
  if (_isReactive(value)) {
    effect(() => _applyAttr(el, name, value.val));
  } else if (typeof value === 'function') {
    effect(() => _applyAttr(el, name, value()));
  } else {
    _applyAttr(el, name, value);
  }
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

function _wrapEventHandler(modifiers, handler) {
  const keyFilters = modifiers.filter(m => m in KEY_MODIFIERS);
  const hasPrevent = modifiers.includes('prevent');
  const hasStop = modifiers.includes('stop');
  const throttleMs = modifiers.includes('throttle') ? 100 : 0;
  const debounceMs = modifiers.includes('debounce') ? 100 : 0;

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

  for (let i = 0; i < _template.parts.length; i++) {
    const part = _template.parts[i];
    const target = targets[i];
    const value = _values[i];

    switch (part.type) {
      case 'attr':  _commitAttr(target, part.name, value); break;
      case 'event': _commitEvent(target, part.event, part.modifiers, value); break;
      case 'ref':   _commitRef(target, value); break;
      case 'node':  _commitNode(target, value); break;
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
 * @returns {EachMarker<T>}
 */
export function each(sig, renderFn) {
  return { _isEach: true, signal: sig, renderFn };
}
