const ELEMENT_NODE = 1;
const TEXT_NODE = 3;
const COMMENT_NODE = 8;
const DOCUMENT_FRAGMENT_NODE = 11;

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

    appendChild(child) { return _appendChild(this, child); },
    insertBefore(child, ref) { return _insertBefore(this, child, ref); },
    removeChild(child) { return _removeChild(this, child); },
    cloneNode(deep = false) { return _cloneNode(this, deep); },
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

export const document = {
  createElement,
  createTextNode,
  createComment,
  createDocumentFragment,
};

if (typeof globalThis.document === 'undefined') {
  globalThis.document = document;
}
