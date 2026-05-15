const ELEMENT_NODE = 1;
const TEXT_NODE = 3;
const COMMENT_NODE = 8;
const DOCUMENT_FRAGMENT_NODE = 11;

/**
 * Parse a compound selector string into a descriptor object.
 * @param {string} selector
 * @returns {{ tag: string|null, id: string|null, classes: string[], attrs: Array<{name: string, value: string|null}> }}
 */
function _parseSelector(selector) {
  if (selector === "") throw new Error("dom-shim: empty selector");
  const result = { tag: null, id: null, classes: [], attrs: [] };
  let i = 0;

  /** @param {number} pos @param {string} reason */
  function malformed(pos, reason) {
    throw new Error(`dom-shim: malformed selector "${selector}" at position ${pos} (${reason})`);
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
 * @param {object} node
 * @param {string} selector
 * @returns {boolean}
 */
function _matchSelector(node, selector) {
  const parsed = _parseSelector(selector);
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

function createElement(tagName) {
  const el = {
    nodeType: ELEMENT_NODE,
    tagName: tagName.toUpperCase(),
    get nodeName() { return this.tagName; },
    attributes: new Map(),
    childNodes: [],
    parentNode: null,
    _listeners: new Map(),

    setAttribute(name, value) { this.attributes.set(name, String(value)); },
    removeAttribute(name) { this.attributes.delete(name); },
    getAttribute(name) { return this.attributes.has(name) ? this.attributes.get(name) : null; },
    hasAttribute(name) { return this.attributes.has(name); },

    addEventListener(event, handler, options) {
      if (!this._listeners.has(event)) this._listeners.set(event, []);
      this._listeners.get(event).push({ handler, once: options?.once ?? false });
    },
    removeEventListener(event, handler) {
      if (!this._listeners.has(event)) return;
      const list = this._listeners.get(event).filter(e => e.handler !== handler);
      this._listeners.set(event, list);
    },
    dispatchEvent(event) {
      const list = this._listeners.get(event.type);
      if (!list) return;
      const toRemove = [];
      for (const entry of [...list]) {
        entry.handler(event);
        if (entry.once) toRemove.push(entry.handler);
      }
      for (const h of toRemove) this.removeEventListener(event.type, h);
    },

    querySelector(selector) {
      let found = null;
      _walkDescendants(this, node => {
        if (!found && node.nodeType === ELEMENT_NODE && _matchSelector(node, selector)) found = node;
      });
      return found;
    },
    querySelectorAll(selector) {
      const results = [];
      _walkDescendants(this, node => {
        if (node.nodeType === ELEMENT_NODE && _matchSelector(node, selector)) results.push(node);
      });
      return results;
    },
    closest(selector) {
      let node = this;
      while (node && node.nodeType === ELEMENT_NODE) {
        if (_matchSelector(node, selector)) return node;
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

    // No-ops present on real HTMLElement; component code routinely calls these
    // after DOM mutations and they must not throw under the test shim.
    focus() {},
    blur() {},
    click() {},
    scrollIntoView() {},
  };
  _applySiblingGetters(el, () => el.parentNode);
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

function _makeEventTarget() {
  const _listeners = new Map();
  return {
    _listeners,
    addEventListener(event, handler, options) {
      if (!_listeners.has(event)) _listeners.set(event, []);
      _listeners.get(event).push({ handler, once: options?.once ?? false });
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

export const document = Object.assign(
  {
    createElement,
    createTextNode,
    createComment,
    createDocumentFragment,
    childNodes: [],
    appendChild(child) { return _appendChild(this, child); },
    querySelector(selector) {
      let found = null;
      _walkDescendants(this, node => {
        if (!found && node.nodeType === ELEMENT_NODE && _matchSelector(node, selector)) found = node;
      });
      return found;
    },
    querySelectorAll(selector) {
      const results = [];
      _walkDescendants(this, node => {
        if (node.nodeType === ELEMENT_NODE && _matchSelector(node, selector)) results.push(node);
      });
      return results;
    },
  },
  _makeEventTarget(),
);

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

// Forward-reference alias so _history.back/forward can reference window after construction.
const exports_window = window;

if (typeof globalThis.document === 'undefined') {
  globalThis.document = document;
}

if (typeof globalThis.window === 'undefined') {
  globalThis.window = window;
}
