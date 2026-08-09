// Canonical presentation state for settings moved between Guided and Pro.
// This module tracks resolved defaults and dirty/provenance state separately
// from HTML defaultValue so async recommendations and relocated controls do
// not lose their baseline. It owns no payload semantics.

function readValue(control) {
  if (!control) return '';
  if (control.type === 'checkbox') return control.checked ? '1' : '0';
  return control.value ?? '';
}

function resolvedDefault(control) {
  if (!control) return '';
  if (control.dataset.wizDefault != null) return control.dataset.wizDefault;
  if (control.dataset.allSettingsDefault != null) return control.dataset.allSettingsDefault;
  const value = control.type === 'checkbox'
    ? (control.defaultChecked ? '1' : '0')
    : control.tagName === 'SELECT'
      ? (Array.from(control.options).find(option => option.defaultSelected)?.value ?? control.options[0]?.value ?? '')
      : control.defaultValue ?? '';
  control.dataset.allSettingsDefault = value;
  return value;
}

function writeValue(control, value) {
  if (!control) return;
  if (control.type === 'checkbox') control.checked = value === true || value === '1';
  else control.value = value ?? '';
}

export class SettingStateRegistry {
  constructor() {
    this.entries = new Map();
  }

  mount(root, descriptors) {
    descriptors.forEach(descriptor => {
      const escapedId = globalThis.CSS?.escape ? CSS.escape(descriptor.id) : descriptor.id.replace(/[^a-zA-Z0-9_-]/g, '\\$&');
      const control = root?.querySelector(`#${escapedId}`) || document.getElementById(descriptor.id);
      if (!control || this.entries.has(descriptor.semanticId)) return;
      const value = readValue(control);
      const defaultValue = resolvedDefault(control);
      this.entries.set(descriptor.semanticId, {
        descriptor,
        control,
        resolvedDefault: defaultValue,
        value,
        dirty: value !== defaultValue,
        provenance: value !== defaultValue ? 'user' : 'resolved',
        effective: null,
        pending: false,
      });
    });
    return this;
  }

  syncControl(control, { provenance = 'user' } = {}) {
    if (!control) return null;
    const entry = [...this.entries.values()].find(item => item.control === control || item.descriptor.id === control.id);
    if (!entry) return null;
    entry.value = readValue(control);
    entry.dirty = entry.value !== entry.resolvedDefault;
    entry.provenance = entry.dirty ? provenance : 'resolved';
    return entry;
  }

  setResolvedDefault(semanticId, value, { applyWhenClean = true } = {}) {
    const entry = this.entries.get(semanticId);
    if (!entry) return false;
    entry.resolvedDefault = value;
    if (applyWhenClean && !entry.dirty) {
      writeValue(entry.control, value);
      entry.value = value;
      entry.provenance = 'resolved';
    }
    entry.dirty = entry.value !== entry.resolvedDefault;
    return true;
  }

  async updateResolvedDefaults(updates, options = {}) {
    const values = await (typeof updates === 'function' ? updates() : updates);
    for (const [semanticId, value] of Object.entries(values || {})) {
      this.setResolvedDefault(semanticId, String(value), options);
    }
    return this;
  }

  setEffective(semanticId, effective, { pending = false } = {}) {
    const entry = this.entries.get(semanticId);
    if (!entry) return false;
    entry.effective = effective;
    entry.pending = pending;
    return true;
  }

  reset(semanticId) {
    const entry = this.entries.get(semanticId);
    if (!entry) return false;
    writeValue(entry.control, entry.resolvedDefault);
    entry.value = entry.resolvedDefault;
    entry.dirty = false;
    entry.provenance = 'resolved';
    entry.pending = false;
    return true;
  }

  changedCount(descriptors) {
    return descriptors.reduce((count, descriptor) => count + (this.entries.get(descriptor.semanticId)?.dirty ? 1 : 0), 0);
  }

  snapshot(descriptors) {
    return descriptors.map(descriptor => {
      const entry = this.entries.get(descriptor.semanticId);
      return entry ? { ...entry, control: undefined } : null;
    }).filter(Boolean);
  }
}

export function createSettingStateRegistry() {
  return new SettingStateRegistry();
}
