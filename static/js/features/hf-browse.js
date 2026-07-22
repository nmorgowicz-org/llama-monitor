// ── HF Browse Module ──────────────────────────────────────────────────────────
// Shared HuggingFace browse, search, file-list, and download utilities.
// Consumed by spawn-wizard.js and (future) models-modal.

import { showToast } from './toast.js';

// ── Discovery scopes (Phase 8B1) ──────────────────────────────────────────────
// Additive toggles: MLX and GGUF can both be active. All = everything (including NVFP4/unsupported).
// Platform defaults: macOS → MLX+GGUF; Win/Linux → GGUF

export const HF_SCOPE = {
  GGUF: 'gguf',
  MLX: 'mlx',
  ALL: 'all',
};

export const HF_SCOPE_LABELS = {
  [HF_SCOPE.GGUF]: 'GGUF',
  [HF_SCOPE.MLX]: 'MLX',
  [HF_SCOPE.ALL]: 'All',
};

// Guide tooltips for scope buttons
export const HF_SCOPE_TOOLTIPS = {
  [HF_SCOPE.MLX]: 'Rapid-MLX native format. Faster inference on Apple Silicon. macOS only.',
  [HF_SCOPE.GGUF]: 'GGUF format. Works everywhere via llama.cpp. Widely available.',
  [HF_SCOPE.ALL]: 'Show everything, including formats not yet supported (NVFP4, etc.).',
};

// ── Sorting modes (Phase 8B1) ─────────────────────────────────────────────────

export const HF_SORT = {
  RELEVANCE: 'relevance',
  NAME: 'name',
  SIZE: 'size',
  LAST_UPDATED: 'last_updated',
  DOWNLOADS: 'downloads',
};

export const HF_SORT_LABELS = {
  [HF_SORT.RELEVANCE]: 'Relevance',
  [HF_SORT.NAME]: 'Name',
  [HF_SORT.SIZE]: 'Size',
  [HF_SORT.LAST_UPDATED]: 'Last updated',
  [HF_SORT.DOWNLOADS]: 'Most downloaded',
};

// ── Category mapping from HF tags (Phase 8B1) ─────────────────────────────────

const HF_TAG_TO_CATEGORY = {
  'text-generation-inference': 'chat',
  'conversational': 'chat',
  'code-generation': 'coding',
  'code': 'coding',
  'instruct': 'chat',
  'chat': 'chat',
  'roleplay': 'roleplay',
  'creative-writing': 'roleplay',
  'story': 'roleplay',
  'role-playing': 'roleplay',
  'function-calling': 'tool-use',
  'tool-use': 'tool-use',
  'mcp': 'tool-use',
  'agentic': 'tool-use',
  'image-to-text': 'vision',
  'multimodal': 'vision',
  'document-question-answering': 'vision',
  'image-text-to-text': 'vision',
};

function resolveCategories(tags) {
  if (!Array.isArray(tags)) return [];
  const cats = new Set();
  for (const tag of tags) {
    const lower = tag.toLowerCase();
    const mapped = HF_TAG_TO_CATEGORY[lower];
    if (mapped) cats.add(mapped);
  }
  return [...cats];
}

// ── Author/converter role (Phase 8B1) ─────────────────────────────────────────

const KNOWN_CONVERTER_PATTERNS = [
  /^bartowski\//, /^mlx-community\//, /^Undi95\//, /^TheBloke\//,
  /^Mradermacher\//, /^cjpais\//, /^lmstudio-community\//,
  /^mrm8488\//, /^runpod\//, /^TuringEnterprises\//, /^Qwen\//,
];

function isLikelyConverter(repoId) {
  return KNOWN_CONVERTER_PATTERNS.some(p => p.test(repoId));
}

function resolveAuthorRole(repoId, tags) {
  const lowerTags = (tags || []).map(t => t.toLowerCase());
  const hasMlxTag = lowerTags.some(t => t.includes('mlx'));
  const hasGgufTag = lowerTags.some(t => t.includes('gguf') || t.includes('gguf-file'));
  const isConverter = isLikelyConverter(repoId);

  if (isConverter) {
    if (hasMlxTag) return { role: 'mlx-converter', label: 'MLX converter' };
    if (hasGgufTag) return { role: 'gguf-quantizer', label: 'GGUF quantizer' };
    return { role: 'converter', label: 'Converter' };
  }

  const parts = (repoId || '').split('/');
  const owner = parts[0] || '';
  const repoName = (parts[1] || '').toLowerCase();

  if (owner && repoName && !repoName.includes('-gguf') && !repoName.includes('-mlx') && !repoName.includes('quant')) {
    return { role: 'original-author', label: 'Original author' };
  }

  return null;
}

// ── Discover categories ───────────────────────────────────────────────────────

export const HF_DISCOVER_CATEGORIES = [
  { id: 'trending',  label: 'Trending',      params: { query: '',           sort: 'trending',  limit: 30 } },
  { id: 'qwen3',     label: 'Qwen3',         params: { query: 'qwen3',      sort: 'downloads', limit: 30 } },
  { id: 'llama3',    label: 'Llama 3.x',     params: { query: 'llama-3',    sort: 'downloads', limit: 30 } },
  { id: 'mistral',   label: 'Mistral / MoE', params: { query: 'mistral',    sort: 'downloads', limit: 30 } },
  { id: 'gemma',     label: 'Gemma',         params: { query: 'gemma',      sort: 'downloads', limit: 30 } },
  { id: 'exaone',    label: 'EXAONE',        params: { query: 'exaone',     sort: 'downloads', limit: 30 } },
  { id: 'heretic',   label: 'Heretic',       params: { query: 'heretic',    sort: 'downloads', limit: 30 } },
];

// ── Small utilities ───────────────────────────────────────────────────────────

function escHtml(s) {
  return String(s).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
}

function hfRelativeAge(iso) {
  if (!iso) return '';
  const ms = Date.now() - new Date(iso).getTime();
  if (isNaN(ms) || ms < 0) return '';
  const mins  = Math.floor(ms / 60_000);
  const hours = Math.floor(ms / 3_600_000);
  const days  = Math.floor(ms / 86_400_000);
  const weeks = Math.floor(days / 7);
  const months = Math.floor(days / 30);
  const years  = Math.floor(days / 365);
  if (mins  <  60)  return `${mins}m ago`;
  if (hours < 24)   return `${hours}h ago`;
  if (days  <  7)   return `${days}d ago`;
  if (weeks <  5)   return `${weeks}w ago`;
  if (months < 12)  return `${months}mo ago`;
  return `${years}y ago`;
}

function formatBytes(bytes) {
  if (!bytes) return '';
  const b = Number(bytes);
  if (!isFinite(b)) return '';
  if (b >= 1024 ** 3) return (b / (1024 ** 3)).toFixed(1) + ' GiB';
  if (b >= 1024 ** 2) return (b / (1024 ** 2)).toFixed(1) + ' MiB';
  if (b >= 1024) return (b / 1024).toFixed(0) + ' KiB';
  return b + ' B';
}

function getRecommendedQuant(vramGb) {
  if (vramGb < 8)  return 'Q4_K_M';
  if (vramGb <= 16) return 'Q5_K_M';
  if (vramGb <= 24) return 'Q5_K_M';
  return 'Q8_0';
}

function getAuthHeaders() {
  return window.authHeaders ? window.authHeaders() : {};
}

// ── Phase 8B2: Base model name extraction ────────────────────────────────────
// Extract canonical base model name from repo_id for grouping.
// E.g., "bartowski/Qwen3-32B-GGUF" -> "Qwen3-32B"
// E.g., "mlx-community/Qwen3-30B-A3B-4bit" -> "Qwen3-30B-A3B"
// E.g., "unsloth/Qwen3.6-32B-heretic" -> "Qwen3.6-32B-heretic"

function extractBaseModelName(repoId) {
  const parts = (repoId || '').split('/');
  const name = (parts[1] || '').toLowerCase();
  if (!name) return repoId;

  // Remove GGUF suffix
  let base = name.replace(/-gguf$/, '');

  // Remove known quant suffixes from MLX repos (e.g., "-4bit", "-8bit")
  base = base.replace(/-(?:4bit|8bit|fp16|q4|q8)$/, '');

  // Remove known converter prefixes from repo name
  base = base.replace(/^(?:mlx-|gguf-)/, '');

  // Capitalize sensibly: preserve known brand casing
  const lower = base;
  let result = '';
  let i = 0;
  while (i < lower.length) {
    const c = lower[i];
    if (c === '-' || c === '.') {
      result += c;
      i++;
    } else if (result.length === 0 || (result[result.length - 1] === '-' || result[result.length - 1] === '.')) {
      result += c.toUpperCase();
      i++;
    } else {
      result += c;
      i++;
    }
  }

  return result || repoId;
}

// ── Phase 8B2: MLX lineage discovery ──────────────────────────────────────────
// Fetch MLX derivatives and conversion recipes for a source repo.

async function fetchMlxLineage(repoId) {
  try {
    const headers = { ...getAuthHeaders(), 'Content-Type': 'application/json' };
    const resp = await fetch('/api/hf/mlx-derivatives', {
      method: 'POST',
      headers,
      body: JSON.stringify({ repoId }),
    });
    if (!resp.ok) return null;
    return await resp.json();
  } catch {
    return null;
  }
}

// ── Phase 8B2: Selection payload builder ──────────────────────────────────────
// Build structured selection payload with repoId/revision/variant/lineage.
// This survives through: selection → estimate → download → library → launch.

export function buildSelectionPayload(model, variantInfo) {
  return {
    repoId: model.id || '',
    revision: model.revision || variantInfo?.revision || null,
    variant: variantInfo?.variant || '',
    originalAuthor: model.original_author || variantInfo?.originalAuthor || null,
    converter: model.converter || variantInfo?.converter || null,
    format: variantInfo?.format || model.format || 'gguf',
    backendHint: variantInfo?.backendHint || null,
    hfFilePath: model.hf_file_path || variantInfo?.hfFilePath || null,
    modelSizeBytes: model.model_size_bytes || variantInfo?.modelSizeBytes || null,
    // Preserve raw model object for backward compatibility
    _raw: model,
  };
}

// ── Scope resolution (Phase 8B1) ──────────────────────────────────────────────
// Additive toggles: MLX and GGUF can both be active. All = everything (including NVFP4/unsupported).

async function resolveScope({ mlxActive, ggufActive, allActive }) {
  if (allActive) {
    return { format: 'all', includeUnsupported: true };
  }
  if (mlxActive && ggufActive) {
    return { format: 'both', includeUnsupported: false };
  }
  if (mlxActive) {
    return { format: 'mlx', includeUnsupported: false };
  }
  // Default to GGUF if neither explicitly active
  return { format: 'gguf', includeUnsupported: false };
}

let _cachedPlatformBackend = null;

async function detectPlatformBackend() {
  if (_cachedPlatformBackend !== null) return _cachedPlatformBackend;

  try {
    const headers = getAuthHeaders();
    const resp = await fetch('/api/llama-binary/platform-info', { headers });
    if (resp.ok) {
      const data = await resp.json();
      _cachedPlatformBackend = data.rapid_mlx_local_available ? 'rapid_mlx' : 'llama_cpp';
      return _cachedPlatformBackend;
    }
  } catch {
    // non-fatal; default to llama_cpp
  }

  _cachedPlatformBackend = 'llama_cpp';
  return 'llama_cpp';
}

// ── Sort resolution (Phase 8B1) ──────────────────────────────────────────────
// Maps our sort modes to backend params. Auto = workload-profile-aware relevance.

function resolveSortParam(legacySort, hfSort) {
  if (hfSort === HF_SORT.NAME) return 'createdAt';
  if (hfSort === HF_SORT.SIZE) return 'downloads';
  if (hfSort === HF_SORT.LAST_UPDATED) return 'createdAt';
  if (hfSort === HF_SORT.RELEVANCE) return 'downloads';
  if (hfSort === HF_SORT.AUTO) return 'downloads';
  return legacySort || 'downloads';
}

// ── hfSearch ──────────────────────────────────────────────────────────────────
// Search HuggingFace models and render results into container.
//
// params:
//   query, author, sort, limit          – search params
//   scope                               – discovery scope (HF_SCOPE.AUTO/GGUF/MLX/ALL)
//   hfSort                              – sorting mode (HF_SORT.AUTO/RELEVANCE/NAME/SIZE/LAST_UPDATED)
//   minParamB                           – minimum parameter count filter
//   cursor                              – pagination cursor
//   append                              – append results instead of replacing
//   container                           – DOM element to render into
//   filelistContainer                   – optional element to hide when showing results
//   quickpicksContainer                 – optional element holding quick-pick buttons (for loading/active state)
//   discoverPillsContainerId            – optional id of discover-pills container (for loading/active state)
//   onOpenCardPanel                     – (repoId) => void
//   onSelectModel                       – (model) => void  (called when user clicks a result row)
//   workloadProfile                     – current workload profile ID (for Auto sorting)

export async function hfSearch({
  query,
  author,
  sort,
  limit,
  mlxActive = true,
  ggufActive = true,
  allActive = false,
  hfSort = HF_SORT.DOWNLOADS,
  minParamB = 0,
  cursor = null,
  append = false,
  _cascadeDepth = 0,
  container,
  filelistContainer,
  quickpicksContainer,
  discoverPillsContainerId,
  onOpenCardPanel,
  onSelectModel,
  workloadProfile,
}) {
  if (!container) return;

  if (!append) {
    container.innerHTML = '<div class="hf-search-loading">Searching HuggingFace…</div>';
    container.style.display = '';
    if (filelistContainer) {
      filelistContainer.innerHTML = '';
      filelistContainer.classList.remove('visible');
    }
  } else {
    // Remove existing load-more button so we can append fresh results + new button
    container.querySelector('.hf-load-more-btn')?.remove();
    const loadingEl = document.createElement('div');
    loadingEl.className = 'hf-search-loading hf-load-more-loading';
    loadingEl.textContent = 'Loading more…';
    container.appendChild(loadingEl);
  }

  const clearPillLoading = () => {
    quickpicksContainer?.querySelectorAll('.hf-qp-btn').forEach(b => b.classList.remove('loading'));
    document.getElementById(discoverPillsContainerId)
      ?.querySelectorAll('.hf-discover-pill').forEach(p => p.classList.remove('loading'));
  };

  // Find the nearest overflow-scrollable ancestor
  const scrollToResults = () => {
    let p = container.parentElement;
    while (p) {
      const s = getComputedStyle(p);
      if ((s.overflowY === 'auto' || s.overflowY === 'scroll') && p.scrollHeight > p.clientHeight) {
        const cRect = container.getBoundingClientRect();
        const pRect = p.getBoundingClientRect();
        p.scrollTo({ top: p.scrollTop + cRect.top - pRect.top - 12, behavior: 'smooth' });
        return;
      }
      p = p.parentElement;
    }
  };

  try {
    // Map discovery scope to backend format param.
    // MLX+GGUF both active = both formats; All = everything including NVFP4/unsupported.
    const resolvedScope = await resolveScope({ mlxActive, ggufActive, allActive });
    const body = {
      query: query || '',
      author: author || undefined,
      sort: resolveSortParam(sort, hfSort),
      limit: limit || 20,
      cursor: cursor || undefined,
      format: resolvedScope.format,
      includeUnsupported: resolvedScope.includeUnsupported,
      workload_profile: workloadProfile || null,
    };

    const headers = { ...getAuthHeaders(), 'Content-Type': 'application/json' };
    const resp = await fetch('/api/hf/search', { method: 'POST', headers, body: JSON.stringify(body) });
    if (!resp.ok) {
      clearPillLoading();
      if (append) {
        container.querySelector('.hf-load-more-loading')?.remove();
      } else {
        container.innerHTML = '<div class="hf-search-empty">Search failed.</div>';
      }
      return;
    }
    const data = await resp.json();
    const allModels = data.models || [];
    const nextCursor = data.next_cursor || null;
    const hasMore = !!nextCursor;

    clearPillLoading();

    // Remove "loading more" placeholder if appending
    if (append) container.querySelector('.hf-load-more-loading')?.remove();
    else container.innerHTML = '';

    // Apply client-side param_b filter
    const models = minParamB > 0 ? allModels.filter(m => (m.param_b || 0) >= minParamB) : allModels;

    if (!models.length && !append) {
      const msg = minParamB > 0
        ? `No models \u2265${minParamB}B found. Try a lower size filter or "Load more".`
        : 'No models found.';
      const empty = document.createElement('div');
      empty.className = 'hf-search-empty';
      empty.textContent = msg;
      container.replaceChildren(empty);
      if (hasMore) {
        const moreBtn = _makeLoadMoreBtn(() => hfSearch({
          query, author, sort, limit, mlxActive, ggufActive, allActive, hfSort, minParamB, cursor: nextCursor, append: true,
          container, filelistContainer, quickpicksContainer,
          discoverPillsContainerId, onOpenCardPanel, onSelectModel,
          workloadProfile,
        }));
        container.appendChild(moreBtn);
      }
      if (!append) scrollToResults();
      return;
    }

    // ── Phase 8B2: Two-level card hierarchy (group + variants) ─────────────────
    // Group models by base model name, render group header + variant rows.

    const groups = new Map();
    for (const m of models) {
      const baseName = extractBaseModelName(m.id);
      if (!groups.has(baseName)) {
        groups.set(baseName, { baseName, models: [], tags: new Set(), author: null });
      }
      const g = groups.get(baseName);
      g.models.push(m);
      (m.tags || []).forEach(t => g.tags.add(t));
      // Use original-author role as group author if present
      if (!g.author && m.id.split('/')[0]) {
        const role = resolveAuthorRole(m.id, m.tags);
        if (role && role.role === 'original-author') {
          g.author = m.id.split('/')[0];
        }
      }
    }

    // Sort groups: Community Picks first, then by first model's downloads
    const sortedGroups = [...groups.values()].sort((a, b) => {
      const aHasPick = a.models.some(m => m.is_community_pick || (m.community_picks || []).length > 0);
      const bHasPick = b.models.some(m => m.is_community_pick || (m.community_picks || []).length > 0);
      if (aHasPick !== bHasPick) return aHasPick ? -1 : 1;
      const aDl = (a.models[0]?.downloads || 0);
      const bDl = (b.models[0]?.downloads || 0);
      return bDl - aDl;
    });

    // Show loading indicator for lineage/enrichment
    const enrichingEl = document.createElement('div');
    enrichingEl.className = 'hf-search-loading';
    enrichingEl.textContent = 'Checking model lineage…';
    container.appendChild(enrichingEl);

    // Fetch identity for each group to get original author (concurrent, bounded)
    const identityTasks = [];
    for (const g of sortedGroups) {
      if (g.models.length > 0) {
        identityTasks.push(
          (async (group) => {
            const firstModel = group.models[0];
            try {
              const headers = { ...getAuthHeaders(), 'Content-Type': 'application/json' };
              const resp = await fetch('/api/hf/identity', {
                method: 'POST',
                headers,
                body: JSON.stringify({ repoId: firstModel.id }),
              });
              if (resp.ok) {
                const data = await resp.json();
                group.originalAuthor = data.original_author?.username || data.original_author?.display_name || null;
                group.isFinetune = !!data.is_finetune;
              }
            } catch {
              // Non-fatal: fall back to heuristic author from Phase 8B1
            }
            group.author = group.originalAuthor || group.author || firstModel.id.split('/')[0];
          })(g)
        );
      }
    }

    // Fetch MLX lineage for groups when MLX is active or All is active (bounded concurrency)
    const mlxTasks = [];
    if (mlxActive || allActive) {
      for (let i = 0; i < sortedGroups.length; i++) {
        const g = sortedGroups[i];
        // Only fetch MLX lineage for finetunes or models with >5 downloads (signal of maturity)
        if (g.isFinetune || (g.models[0]?.downloads || 0) > 5) {
          mlxTasks.push(
            (async (group) => {
              group.mlxLineage = await fetchMlxLineage(g.models[0].id);
            })(g)
          );
        }
      }
    }

    // Wait for enrichment with timeout (don't block too long)
    try {
      await Promise.race([
        Promise.all([...identityTasks, ...mlxTasks]),
        new Promise(r => setTimeout(r, 4000)),
      ]);
    } catch {
      // Non-fatal: proceed with partial data
    }

    // Remove enriching indicator
    enrichingEl.remove();

    // Render each group
    for (const g of sortedGroups) {
      const groupEl = document.createElement('div');
      groupEl.className = 'hf-search-group';
      groupEl.dataset.baseName = g.baseName;

      // ── Group header ───────────────────────────────────────────────────────
      const header = document.createElement('div');
      header.className = 'hf-sg-header';

      // Base model name + original author
      const headerNameLine = document.createElement('div');
      headerNameLine.className = 'hf-sg-header-name';

      const baseNameEl = document.createElement('span');
      baseNameEl.className = 'hf-sg-base-name';
      baseNameEl.textContent = g.baseName;
      headerNameLine.appendChild(baseNameEl);

      // Original author badge
      if (g.originalAuthor && g.originalAuthor !== g.author) {
        const origBadge = document.createElement('span');
        origBadge.className = 'hf-sg-role-badge hf-sg-role-badge--original-author';
        origBadge.textContent = 'by ' + escHtml(g.originalAuthor);
        origBadge.title = 'Original author';
        headerNameLine.appendChild(origBadge);
      } else if (g.author) {
        const origBadge = document.createElement('span');
        origBadge.className = 'hf-sg-role-badge hf-sg-role-badge--original-author';
        origBadge.textContent = 'by ' + escHtml(g.author);
        origBadge.title = 'Original author';
        headerNameLine.appendChild(origBadge);
      }

      // Community Picks badge if any variant is a pick
      const hasPick = g.models.some(m => m.is_community_pick || (m.community_picks || []).length > 0);
      if (hasPick) {
        const cpBadge = document.createElement('span');
        cpBadge.className = 'hf-sg-role-badge hf-sg-role-badge--community-pick';
        cpBadge.textContent = '★ Community Pick';
        cpBadge.title = 'High-quality repo selected by the community';
        headerNameLine.appendChild(cpBadge);
      }

      header.appendChild(headerNameLine);

      // Categories
      const groupCategories = resolveCategories([...g.tags]);
      if (groupCategories.length > 0) {
        const catRail = document.createElement('div');
        catRail.className = 'hf-sg-categories';
        groupCategories.forEach(cat => {
          const pill = document.createElement('span');
          pill.className = `hf-sr-category hf-sr-category--${cat}`;
          pill.textContent = cat.charAt(0).toUpperCase() + cat.slice(1);
          catRail.appendChild(pill);
        });
        header.appendChild(catRail);
      }

      // Expand/collapse toggle
      const toggle = document.createElement('button');
      toggle.type = 'button';
      toggle.className = 'hf-sg-toggle';
      toggle.textContent = '▾';
      toggle.setAttribute('aria-label', 'Toggle variants');
      toggle.addEventListener('click', () => {
        const body = groupEl.querySelector('.hf-sg-body');
        if (body.style.display === 'none') {
          body.style.display = '';
          toggle.textContent = '▾';
        } else {
          body.style.display = 'none';
          toggle.textContent = '▸';
        }
      });
      header.appendChild(toggle);

      groupEl.appendChild(header);

      // ── Group body (variants list) ─────────────────────────────────────────
      const body = document.createElement('div');
      body.className = 'hf-sg-body';

      // Variant rows for each model in the group
      for (const m of g.models) {
        const variantRow = document.createElement('div');
        variantRow.className = 'hf-sg-variant';
        variantRow.setAttribute('tabindex', '0');
        variantRow.setAttribute('role', 'button');

        // Variant name (repo id with converter prefix)
        const variantName = document.createElement('span');
        variantName.className = 'hf-sg-variant-name';
        variantName.textContent = m.id;
        variantRow.appendChild(variantName);

        // Format badge — detect from repo name/ID (never trust HF tags)
        const repoIdLower = (m.id || '').toLowerCase();
        const isMlx = repoIdLower.includes('.mlx') ||
          repoIdLower.includes('/mlx/') ||
          repoIdLower.includes('-mlx-') ||
          repoIdLower.endsWith('-mlx') ||
          repoIdLower.includes('.safetensors');
        const isGguf = repoIdLower.includes('.gguf') || repoIdLower.includes('-gguf') || repoIdLower.includes('/gguf/');
        // GGUF repo name takes priority over safetensors (many MLX repos host GGUF versions with -GGUF suffix)
        const format = isGguf ? 'gguf' : isMlx ? 'mlx' : 'unknown';
        const formatBadge = document.createElement('span');
        formatBadge.className = `hf-sg-format-badge hf-sg-format-badge--${format}`;
        formatBadge.textContent = format.toUpperCase();
        formatBadge.title = `Format: ${format.toUpperCase()}`;
        variantRow.appendChild(formatBadge);

        // Quant label / size
        const variantMeta = document.createElement('span');
        variantMeta.className = 'hf-sg-variant-meta';
        const metaParts = [];
        if (m.quant_label) metaParts.push(m.quant_label);
        if (m.model_size_bytes) metaParts.push(formatBytes(m.model_size_bytes));
        if (m.downloads > 0) metaParts.push(m.downloads >= 1000 ? `${(m.downloads / 1000).toFixed(0)}k↓` : `${m.downloads}↓`);
        variantMeta.textContent = metaParts.join(' · ');
        variantRow.appendChild(variantMeta);

        // Converter role badge
        const converterRole = resolveAuthorRole(m.id, m.tags);
        if (converterRole && converterRole.role !== 'original-author') {
          const roleBadge = document.createElement('span');
          roleBadge.className = `hf-sg-role-badge hf-sg-role-badge--${converterRole.role}`;
          roleBadge.textContent = escHtml(m.id.split('/')[0]);
          roleBadge.title = converterRole.label + ' — ' + m.id.split('/')[0];
          variantRow.appendChild(roleBadge);
        }

        // Card link button
        const cardLink = document.createElement('button');
        cardLink.type = 'button';
        cardLink.className = 'hf-sg-card-link';
        cardLink.title = 'View model card';
        cardLink.setAttribute('aria-label', `View model card for ${escHtml(m.id)}`);
        cardLink.innerHTML =
          '<svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><polyline points="14 2 14 8 20 8"/><line x1="16" y1="13" x2="8" y2="13"/><line x1="16" y1="17" x2="8" y2="17"/><polyline points="10 9 9 9 8 9"/></svg>';
        cardLink.addEventListener('click', e => {
          e.stopPropagation();
          if (onOpenCardPanel) onOpenCardPanel(m.id);
          else window.open(`https://huggingface.co/${escHtml(m.id)}`, '_blank', 'noopener');
        });
        variantRow.appendChild(cardLink);

        // Selection handler with enriched payload
        const selectVariant = () => {
          if (onSelectModel) {
            const payload = buildSelectionPayload(m, {
              format,
              converter: converterRole ? m.id.split('/')[0] : null,
              originalAuthor: g.originalAuthor || g.author || null,
            });
            onSelectModel(payload);
          }
        };
        variantRow.addEventListener('click', selectVariant);
        variantRow.addEventListener('keydown', e => {
          if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); selectVariant(); }
        });

        body.appendChild(variantRow);
      }

      // ── MLX lineage section (Phase 8B2: native + converted) ───────────────
      if (g.mlxLineage && (mlxActive || allActive)) {
        const lineageSection = document.createElement('div');
        lineageSection.className = 'hf-sg-mlx-lineage';

        // Native MLX derivatives
        const nativeDerivs = (g.mlxLineage.native_mlx_derivatives || []).slice(0, 3);
        if (nativeDerivs.length > 0) {
          const nativeHeader = document.createElement('div');
          nativeHeader.className = 'hf-sg-mlx-section-label';
          nativeHeader.textContent = 'MLX variants';
          lineageSection.appendChild(nativeHeader);

          for (const d of nativeDerivs) {
            const mlxRow = document.createElement('div');
            mlxRow.className = 'hf-sg-mlx-variant';
            mlxRow.setAttribute('tabindex', '0');
            mlxRow.setAttribute('role', 'button');

            const mlxName = document.createElement('span');
            mlxName.className = 'hf-sg-mlx-variant-name';
            mlxName.textContent = d.repo_id;
            mlxRow.appendChild(mlxName);

            // Native vs converted label
            const isKnownPublisher = ['mlx-community', 'ml-explore'].includes(d.converter);
            const mlxLabel = document.createElement('span');
            mlxLabel.className = `hf-sg-mlx-label ${isKnownPublisher ? 'hf-sg-mlx-label--native' : 'hf-sg-mlx-label--converted'}`;
            mlxLabel.textContent = isKnownPublisher
              ? `Native MLX by ${escHtml(d.converter)}`
              : `Converted by ${escHtml(d.converter)}`;
            mlxRow.appendChild(mlxLabel);

            // Size
            if (d.size > 0) {
              const mlxSize = document.createElement('span');
              mlxSize.className = 'hf-sg-mlx-variant-meta';
              mlxSize.textContent = formatBytes(d.size);
              mlxRow.appendChild(mlxSize);
            }

            const selectMlx = () => {
              if (onSelectModel) {
                const payload = buildSelectionPayload(
                  { id: d.repo_id, revision: d.revision, model_size_bytes: d.size },
                  {
                    format: 'mlx',
                    converter: d.converter,
                    originalAuthor: g.originalAuthor || g.author || g.mlxLineage.original_author || null,
                    backendHint: 'rapid_mlx',
                  }
                );
                onSelectModel(payload);
              }
            };
            mlxRow.addEventListener('click', selectMlx);
            mlxRow.addEventListener('keydown', e => {
              if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); selectMlx(); }
            });

            lineageSection.appendChild(mlxRow);
          }
        }

        // Conversion candidates
        const recipes = (g.mlxLineage.conversion_recipes || []).slice(0, 2);
        if (recipes.length > 0) {
          const convHeader = document.createElement('div');
          convHeader.className = 'hf-sg-mlx-section-label';
          convHeader.textContent = 'Conversion options';
          lineageSection.appendChild(convHeader);

          for (const r of recipes) {
            const convRow = document.createElement('div');
            convRow.className = 'hf-sg-mlx-recipe';
            const convName = document.createElement('span');
            convName.className = 'hf-sg-mlx-recipe-name';
            convName.textContent = r.recipe;
            convRow.appendChild(convName);
            const convMeta = document.createElement('span');
            convMeta.className = 'hf-sg-mlx-recipe-meta';
            const parts = [];
            if (r.input_format) parts.push(r.input_format);
            if (r.estimated_time) parts.push(r.estimated_time);
            convMeta.textContent = parts.join(' · ');
            convRow.appendChild(convMeta);
            convRow.title = r.description || '';

            // Conversion candidates show recipe info; clicking opens info (not auto-select)
            const convLink = document.createElement('button');
            convLink.type = 'button';
            convLink.className = 'hf-sg-card-link';
            convLink.title = r.description || 'View conversion details';
            convLink.innerHTML = '<svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><path d="M12 16v-4"/><path d="M12 8h.01"/></svg>';
            convLink.addEventListener('click', e => {
              e.stopPropagation();
              showToast(`Conversion: ${r.recipe} — ${r.description}`, 'info');
            });
            convRow.appendChild(convLink);

            lineageSection.appendChild(convRow);
          }
        }

        body.appendChild(lineageSection);
      }

      groupEl.appendChild(body);
      container.appendChild(groupEl);
    }

    if (hasMore) {
      const moreBtn = _makeLoadMoreBtn(() => hfSearch({
        query, author, sort, limit, mlxActive, ggufActive, allActive, hfSort, minParamB, cursor: nextCursor, append: true,
        _cascadeDepth: 0,
        container, filelistContainer, quickpicksContainer,
        discoverPillsContainerId, onOpenCardPanel, onSelectModel,
        workloadProfile,
      }));
      container.appendChild(moreBtn);
    }

    if (!append) scrollToResults();

    // Auto-cascade: if fewer than 10 results are visible and there are more pages,
    // silently fetch the next page (capped at 3 auto-fetches to avoid runaway).
    const visibleCount = container.querySelectorAll('.hf-search-result').length;
    if (hasMore && visibleCount < 10 && _cascadeDepth < 3) {
      hfSearch({
        query, author, sort, limit, mlxActive, ggufActive, allActive, hfSort, minParamB, cursor: nextCursor, append: true,
        _cascadeDepth: _cascadeDepth + 1,
        container, filelistContainer, quickpicksContainer,
        discoverPillsContainerId, onOpenCardPanel, onSelectModel,
        workloadProfile,
      });
    }
  } catch (err) {
    clearPillLoading();
    if (append) {
      container.querySelector('.hf-load-more-loading')?.remove();
    } else {
      const errEl = document.createElement('div');
      errEl.className = 'hf-search-empty';
      errEl.textContent = 'Error: ' + (err.message || String(err));
      container.appendChild(errEl);
    }
  }
}

function _makeLoadMoreBtn(onClick) {
  const btn = document.createElement('button');
  btn.type = 'button';
  btn.className = 'hf-load-more-btn';
  btn.textContent = 'Load more';
  btn.addEventListener('click', onClick);
  return btn;
}

// ── hfListFiles ───────────────────────────────────────────────────────────────
// List GGUF files for a repo and render into container.
//
// params:
//   repoId                              – HF repo ID
//   container                           – DOM element to render into
//   vramGb                              – available VRAM in GiB (for recommendation badge)
//   onOpenCardPanel                     – (repoId) => void
//   onSelectFile                        – (file, repoId) => void

export function getRecommendedMmproj(files) {
  return (files || []).find(file => file.is_mmproj && file.is_recommended_mmproj) || null;
}

export async function hfListFiles({
  repoId,
  container,
  vramGb,
  onOpenCardPanel,
  onSelectFile,
}) {
  if (!container) return;

  container.innerHTML = '<div class="hf-file-loading">Loading GGUF files…</div>';
  container.classList.add('visible');

  try {
    const headers = { ...getAuthHeaders(), 'Content-Type': 'application/json' };
    const resp = await fetch('/api/hf/files', {
      method: 'POST',
      headers,
      body: JSON.stringify({ repo_id: repoId }),
    });
    if (!resp.ok) {
      container.innerHTML = '<div class="hf-file-empty">Failed to load files. Check the repo ID.</div>';
      return;
    }
    const data = await resp.json();
    const files = (data.files || []).filter(Boolean);

    container.innerHTML = '';
    if (!files.length) {
      container.innerHTML = '<div class="hf-file-empty">No GGUF files found in this repo.</div>';
      return;
    }

    let autoSelectFn = null;
    let firstSelectFn = null;

    for (const file of files) {
      const fname = file.path || file.name || '';
      if (!fname) continue;

      const item = document.createElement('div');
      item.className = 'hf-file-item';
      item.setAttribute('tabindex', '0');
      item.setAttribute('role', 'button');
      item.dataset.filename = fname;
      item.dataset.repoId = file.repo_id || repoId;
      item.dataset.size = file.size || '';
      item.dataset.label = file.label || '';
      if (file.is_mmproj) item.dataset.mmproj = '1';
      if (file.is_draft_assistant) item.dataset.isDraftModel = '1';
      if (file.is_recommended_mmproj) item.dataset.recommendedMmproj = '1';
      if (file.mmproj_recommendation) {
        item.dataset.mmprojRecommendation = file.mmproj_recommendation;
      }

      const nameSpan = document.createElement('span');
      nameSpan.className = 'hf-file-name';
      const displayName = fname.split('/').pop() || fname;
      nameSpan.textContent = displayName;
      nameSpan.title = displayName;

      const metaSpan = document.createElement('span');
      metaSpan.className = 'hf-file-size';
      const parts = [];
      if (file.size) parts.push(formatBytes(file.size));
      if (file.label) {
        parts.push(file.label);
        if (file.is_recommended_mmproj) {
          parts.push('\u2713 Family recommended');
        } else if (!file.is_mmproj && vramGb > 0 && file.label === getRecommendedQuant(vramGb)) {
          parts.push('\u2713 Recommended');
        }
      }
      metaSpan.textContent = parts.join(' \u00b7 ');

      const qt = file.quant_type || '';
      if (qt === 'imatrix' || file.is_imatrix) {
        const b = document.createElement('span');
        b.className = 'hf-file-badge hf-file-badge-imatrix';
        b.textContent = 'imatrix';
        b.title = 'Importance-matrix calibrated — better quality at same bpw (mradermacher style)';
        nameSpan.appendChild(b);
      } else if (qt === 'unsloth_dynamic') {
        const b = document.createElement('span');
        b.className = 'hf-file-badge hf-file-badge-ud';
        b.textContent = 'UD';
        b.title = 'Unsloth Dynamic — mixed bits per layer, excellent quality/size tradeoff';
        nameSpan.appendChild(b);
      }
      if (file.is_mmproj) {
        const b = document.createElement('span');
        b.className = 'hf-file-badge hf-file-badge-mmproj';
        b.textContent = 'mmproj';
        b.title = 'Vision projector — load alongside the main model for multimodal inference';
        nameSpan.appendChild(b);
        if (file.is_recommended_mmproj) {
          const recommended = document.createElement('span');
          recommended.className = 'hf-file-badge hf-file-badge-recommended';
          recommended.textContent = 'recommended';
          recommended.title = file.mmproj_recommendation || 'Preferred projector format for this model family';
          nameSpan.appendChild(recommended);
        }
      }
      if (file.is_draft_assistant) {
        const b = document.createElement('span');
         b.className = 'hf-file-badge hf-file-badge-draft';
        b.textContent = 'Assistant';
         b.title = 'MTP draft model — use as --model-draft for speculative decoding';
        nameSpan.appendChild(b);
      }

      item.appendChild(nameSpan);
      item.appendChild(metaSpan);

      const selectFile = () => {
        if (onSelectFile) onSelectFile(file, file.repo_id || repoId);
      };
      item.addEventListener('click', selectFile);
      item.addEventListener('keydown', e => {
        if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); selectFile(); }
      });

      if (!file.is_mmproj && !file.is_draft_assistant) {
        if (!firstSelectFn) firstSelectFn = selectFile;
        if (!autoSelectFn && file.label && vramGb > 0 && file.label === getRecommendedQuant(vramGb)) {
          autoSelectFn = selectFile;
        }
      }

      container.appendChild(item);
    }

    // Do NOT auto-select first file on load; let the user pick from the list.
    // Recommendation badges (★) still guide the eye.
  } catch {
    container.innerHTML = '<div class="hf-file-empty">Error loading files. Check the repo ID and your HF token.</div>';
  }
}

// ── hfStartDownload ───────────────────────────────────────────────────────────
// Start a download and show progress in panelEl.
//
// params:
//   repoId, filePath                    – HF repo and file to download
//   panelEl                             – DOM element for the download panel
//   onComplete                          – (downloadId, localPath) => void
//   onValidationError                   – (msg) => void
//   onClearValidationError              – () => void (optional)

export async function hfStartDownload({
  repoId,
  filePath,
  panelEl,
  onComplete,
  onValidationError,
  onClearValidationError,
  companionId,
}) {
  if (!repoId || !filePath) {
    if (onValidationError) onValidationError('Select a GGUF file first.');
    return;
  }
  if (!panelEl) return;

  const btn = panelEl.querySelector('#hf-dlp-download-btn, #mm-hf-dlp-download-btn');
  const shortName = filePath.split('/').pop() || filePath;
  if (btn) {
    btn.disabled = true;
    btn.textContent = 'Starting\u2026';
  }

  try {
    const headers = { ...getAuthHeaders(), 'Content-Type': 'application/json' };
    const res = await fetch('/api/hf/download', {
      method: 'POST',
      headers,
      body: JSON.stringify({ repo_id: repoId, file_path: filePath, resume: true }),
    });
    const data = await res.json().catch(() => ({}));

    if (!res.ok || !data.ok) {
      if (btn) {
        btn.disabled = false;
        btn.textContent = 'Download to models folder';
      }
      const err = data.error || 'Download failed to start.';
      _handleDownloadError(err, shortName, onValidationError);
      return null;
    }

    // Download started: toast + progress UI.
    showToast('Download started: ' + shortName, 'info');

    const downloadId = data.download_id;

    const fileEl = panelEl.querySelector('#hf-dlp-progress-file, #mm-hf-dlp-progress-file');
    if (fileEl) fileEl.textContent = shortName;
    _dlSetState(panelEl, 'progress');
    hfPollDownload(downloadId, panelEl, { onComplete, onValidationError, onClearValidationError, companionId });
    return data;
  } catch (err) {
    if (btn) {
      btn.disabled = false;
      btn.textContent = 'Download to models folder';
    }
    if (onValidationError) onValidationError('Download request failed: ' + (err.message || err));
    return null;
  }
}

/**
 * Centralized handler for download-start errors from the backend.
 * Shows user-friendly toasts and keeps onValidationError in sync.
 */
function _handleDownloadError(err, shortName, onValidationError) {
  const e = (err || '').toLowerCase();

  if (e.includes('already downloading')) {
    const msg = 'Already downloading: ' + shortName + '. Waiting for it to complete.';
    showToast(msg, 'warning');
    if (onValidationError) onValidationError(err || msg);
    return;
  }

  if (e.includes('already exists')) {
    const msg = 'File already exists. It may already be in your library.';
    showToast(msg, 'warning');
    if (onValidationError) onValidationError(err || msg);
    return;
  }

  if (e.includes('too many downloads')) {
    const msg = 'Too many downloads in progress. Please wait for one to finish.';
    showToast(msg, 'warning');
    if (onValidationError) onValidationError(err || msg);
    return;
  }

  // Generic backend error.
  const userMsg = 'Download failed: ' + (err || 'unknown error');
  showToast(userMsg, 'error');
  if (onValidationError) onValidationError(err || userMsg);
}

export async function hfStartCompanionDownload({ repoId, filePath, saveAs }) {
  if (!repoId || !filePath) return null;
  try {
    const headers = { ...getAuthHeaders(), 'Content-Type': 'application/json' };
    const res = await fetch('/api/hf/download', {
      method: 'POST',
      headers,
      body: JSON.stringify({
        repo_id: repoId,
        file_path: filePath,
        save_as: saveAs || undefined,
        companion: true,
        resume: true,
      }),
    });
    const data = await res.json().catch(() => ({}));
    if (res.ok && data.ok) {
      const shortName = (saveAs || filePath.split('/').pop()) || filePath;
      showToast('Also downloading: ' + shortName, 'info');
      return data;
    }
    // Log error quietly for companion; primary handler will surface main message.
    if (data.error) {
      console.warn('[hf] companion download failed:', data.error);
    }
    return null;
  } catch {
    return null;
  }
}

// ── hfPollDownload ────────────────────────────────────────────────────────────
// Poll download status and update progress in panelEl.
// Caller owns state updates via onComplete/onValidationError.

export function hfPollDownload(downloadId, panelEl, { onComplete, onValidationError, onClearValidationError, companionId }) {
  if (!panelEl) return;
  _dlCancelPoll(panelEl);

  const headers = getAuthHeaders();
  let consecutiveErrors = 0;
  const maxConsecutiveErrors = 5;
  let companionResolved = !companionId;

  async function checkCompanion() {
    if (!companionId || companionResolved) return;
    try {
      const cRes = await fetch(`/api/models/download/${companionId}/status`, { headers });
      if (!cRes.ok) return;
      const cData = await cRes.json().catch(() => ({}));
      const cStatus = cData?.status?.status || cData?.status;
      if (!cStatus) return;
      const { status: cStatusVal, message: cMessage } = cStatus;
      companionResolved = true;

      if (cStatusVal === 'completed') {
        const cName = (cStatus.local_path || '').split('/').pop() || 'mmproj';
        showToast('Also downloaded: ' + cName, 'info');
      } else if (cStatusVal === 'failed' || cStatusVal === 'cancelled') {
        const reason = cMessage || 'Companion download failed.';
        showToast('mmproj download issue: ' + reason, 'error');
      }
    } catch {
      // non-fatal
    }
  }

  async function poll() {
    try {
      const res = await fetch(`/api/models/download/${downloadId}/status`, { headers });
      if (!res.ok) {
        consecutiveErrors++;
        if (consecutiveErrors >= maxConsecutiveErrors) {
          _dlCancelPoll(panelEl);
          _dlSetState(panelEl, 'idle');
          if (onValidationError) onValidationError('Unable to check download status. It may have failed or been cancelled.');
          return;
        }
        _dlSchedulePoll(panelEl, poll, 1000);
        return;
      }
      const data = await res.json();
      const s = data.status;
      if (!s) {
        consecutiveErrors++;
        if (consecutiveErrors >= maxConsecutiveErrors) {
          _dlCancelPoll(panelEl);
          _dlSetState(panelEl, 'idle');
          if (onValidationError) onValidationError('Unable to check download status. It may have failed or been cancelled.');
          return;
        }
        _dlSchedulePoll(panelEl, poll, 1000);
        return;
      }

      consecutiveErrors = 0;

      const { status, bytes_downloaded = 0, total_bytes = 0, speed = 0, eta = 0 } = s;
      const pct = total_bytes > 0 ? Math.round(bytes_downloaded / total_bytes * 100) : 0;

      const bar = panelEl.querySelector('#hf-dlp-bar, #mm-hf-dlp-bar');
      if (bar) bar.style.width = pct + '%';

      const pctEl = panelEl.querySelector('#hf-dlp-progress-pct, #mm-hf-dlp-progress-pct');
      if (pctEl) pctEl.textContent = total_bytes > 0 ? `${pct}%` : '';

      const statsEl = panelEl.querySelector('#hf-dlp-stats, #mm-hf-dlp-stats');
      if (statsEl) {
        const mb = (bytes_downloaded / 1_048_576).toFixed(1);
        const tot = total_bytes > 0 ? ` / ${(total_bytes / 1_048_576).toFixed(0)} MB` : '';
        const spd = speed > 0 ? ` \u00b7 ${(speed / 1_048_576).toFixed(1)} MB/s` : '';
        const etaStr = eta > 0
          ? ` \u00b7 ETA ${eta < 60 ? eta + 's' : Math.round(eta / 60) + 'm'}`
          : '';
        statsEl.textContent = `${mb} MB${tot}${spd}${etaStr}`;
      }

      // Also check companion status while main is running
      if (status === 'running' && companionId && !companionResolved) {
        checkCompanion();
      }

      if (status === 'completed') {
        _dlCancelPoll(panelEl);
        _dlSetState(panelEl, 'complete');
        if (onClearValidationError) onClearValidationError();
        const localPath = data.status?.local_path || data.local_path;
        if (onComplete) onComplete(downloadId, localPath);
        return;
      }
      if (status === 'failed') {
        _dlCancelPoll(panelEl);
        _dlSetState(panelEl, 'idle');
        const reason = s.message || 'Download failed.';
        const shortName = data.status?.local_path
            ? (data.status.local_path.split('/').pop() || reason)
            : null;
        const retryable = reason.toLowerCase().includes('connection') ||
            reason.toLowerCase().includes('timed out') ||
            reason.toLowerCase().includes('retry') ||
            reason.toLowerCase().includes('error');
        if (retryable) {
          showToast('Download failed: ' + (shortName ? shortName + ' — ' : '') + reason + ' You can retry.', 'error');
        }
        if (onValidationError) onValidationError(reason);
        return;
      }
      if (status === 'cancelled') {
        _dlCancelPoll(panelEl);
        _dlSetState(panelEl, 'idle');
        return;
      }
    } catch {
      consecutiveErrors++;
      if (consecutiveErrors >= maxConsecutiveErrors) {
        _dlCancelPoll(panelEl);
        _dlSetState(panelEl, 'idle');
        if (onValidationError) onValidationError('Lost connection while checking download status.');
        return;
      }
    }
    _dlSchedulePoll(panelEl, poll, 1000);
  }

  _dlSchedulePoll(panelEl, poll, 800);
}

// ── hfCancelDownload ──────────────────────────────────────────────────────────
// Cancel an active download.

export async function hfCancelDownload({ downloadId, panelEl }) {
  if (!downloadId || !panelEl) return;
  const headers = getAuthHeaders();
  await fetch(`/api/models/download/${downloadId}/cancel`, { method: 'POST', headers }).catch(() => {});
  _dlCancelPoll(panelEl);
  _dlSetState(panelEl, 'idle');
}

// ── hfShowDownloadPanel ───────────────────────────────────────────────────────
// Show the download panel and set the idle state + destination path.

export async function hfShowDownloadPanel(panelEl, fname) {
  if (!panelEl) return;
  _dlSetState(panelEl, 'idle');
  panelEl.style.display = '';

  try {
    const headers = getAuthHeaders();
    const res = await fetch('/api/hf/download-dir', { headers });
    const data = res.ok ? await res.json() : null;
    const dir = data?.dir || '~/.config/llama-monitor/models';
    const configured = data?.configured ?? false;
    const destPath = dir.replace(/\/$/, '') + '/' + (fname || '').split('/').pop();

    const destEl = panelEl.querySelector('.hf-dlp-dest-path');
    if (destEl) { destEl.textContent = destPath; destEl.title = destPath; }

    const warnEl = panelEl.querySelector('.hf-dlp-warn');
    if (warnEl) warnEl.style.display = configured ? 'none' : '';
  } catch { /* ignore */ }
}

export function hfHideDownloadPanel(panelEl) {
  if (!panelEl) return;
  panelEl.style.display = 'none';
  _dlCancelPoll(panelEl);
}

// ── hfRenderDiscoverPills ─────────────────────────────────────────────────────
// Render discover pills into container.
//
// params:
//   container                           – DOM element to render into
//   quickpicksContainer                 – optional element holding quick-pick buttons
//   onPillClick                         – (cat, pillEl) => void  (called when a pill is clicked)

export function hfRenderDiscoverPills({ container, quickpicksContainer, onPillClick }) {
  if (!container) return;
  container.innerHTML = '';

  for (const cat of HF_DISCOVER_CATEGORIES) {
    const pill = document.createElement('button');
    pill.type = 'button';
    pill.className = 'hf-discover-pill';
    pill.textContent = cat.label;
    pill.dataset.catId = cat.id;

    pill.addEventListener('click', () => {
      container.querySelectorAll('.hf-discover-pill').forEach(p => p.classList.remove('active', 'loading'));
      quickpicksContainer?.querySelectorAll('.hf-qp-btn').forEach(b => b.classList.remove('active', 'loading'));
      pill.classList.add('active', 'loading');
      if (onPillClick) onPillClick(cat, pill);
    });

    container.appendChild(pill);
  }
}

// ── hfLoadQuickPicks ──────────────────────────────────────────────────────────
// Load quantizer quick-picks and render into container.
//
// params:
//   container                           – DOM element to render into
//   discoverPillsContainerId            – optional id of discover-pills container
//   onAuthorClick                       – (author, btnEl) => void  (called when a quick-pick is clicked)

export async function hfLoadQuickPicks({ container, discoverPillsContainerId, onAuthorClick }) {
  if (!container) return;
  try {
    const headers = getAuthHeaders();
    const resp = await fetch('/api/hf/quantizers', { headers });
    if (!resp.ok) return;
    const data = await resp.json();
    if (!data.ok || !data.quantizers) return;

    container.innerHTML = '';
    for (const q of data.quantizers) {
      const btn = document.createElement('button');
      btn.type = 'button';
      btn.className = 'hf-qp-btn';
      if (q.quant_style === 'imatrix') btn.classList.add('hf-qp-imatrix');
      if (q.quant_style === 'ud') btn.classList.add('hf-qp-ud');
      btn.textContent = q.display_name;
      btn.title = q.description + (q.note ? `\n\n${q.note}` : '');
      btn.dataset.author = q.username;

      btn.addEventListener('click', () => {
        container.querySelectorAll('.hf-qp-btn').forEach(b => b.classList.remove('active', 'loading'));
        document.getElementById(discoverPillsContainerId)
          ?.querySelectorAll('.hf-discover-pill').forEach(p => p.classList.remove('active', 'loading'));
        btn.classList.add('active', 'loading');
        if (onAuthorClick) onAuthorClick(q.username, btn);
      });

      container.appendChild(btn);
    }
  } catch { /* non-fatal */ }
}

// ── Internal helpers (download panel state) ───────────────────────────────────

function _dlSetState(panelEl, state) {
  ['idle', 'progress', 'complete'].forEach(s => {
    const el = panelEl.querySelector(`#hf-dlp-${s}, #mm-hf-dlp-${s}`);
    if (el) el.style.display = s === state ? '' : 'none';
  });
  if (state === 'idle') {
    const btn = panelEl.querySelector('#hf-dlp-download-btn, #mm-hf-dlp-download-btn');
    if (btn) {
      btn.disabled = false;
      btn.textContent = 'Download to models folder';
    }
  }
}

function _dlSchedulePoll(panelEl, fn, ms) {
  const existing = panelEl._hfDlPollTimer;
  if (existing) clearTimeout(existing);
  panelEl._hfDlPollTimer = setTimeout(fn, ms);
}

function _dlCancelPoll(panelEl) {
  if (panelEl._hfDlPollTimer) {
    clearTimeout(panelEl._hfDlPollTimer);
    panelEl._hfDlPollTimer = null;
  }
}

// ── Discovery scope selector (Phase 8B1) ──────────────────────────────────────
// Additive toggles: MLX and GGUF can both be active. All = everything (including NVFP4/unsupported).
// Returns the selector element. Caller sets initial state via container.dataset.
//
// params:
//   container            – DOM element to append into. Set dataset.hfScopeMlx="1" and/or dataset.hfScopeGguf="1" for initial active scopes.
//   onChange             – (mlxActive, ggufActive, allActive) => void called when scope changes

export function hfCreateScopeSelector({ container, onChange }) {
  if (!container) return null;

  const wrap = document.createElement('div');
  wrap.className = 'hf-scope-selector';
  wrap.style.cssText = 'display:flex;gap:0;border-radius:6px;overflow:hidden;border:1px solid rgba(255,255,255,0.08);';

  const scopes = [
    { key: 'mlx', label: 'MLX', tooltip: HF_SCOPE_TOOLTIPS[HF_SCOPE.MLX] },
    { key: 'gguf', label: 'GGUF', tooltip: HF_SCOPE_TOOLTIPS[HF_SCOPE.GGUF] },
    { key: 'all', label: 'All', tooltip: HF_SCOPE_TOOLTIPS[HF_SCOPE.ALL] },
  ];

  // Read initial state from container dataset
  const initialMlx = container.dataset.hfScopeMlx === '1';
  const initialGguf = container.dataset.hfScopeGguf === '1';
  const initialAll = container.dataset.hfScopeAll === '1';

  // Initialize wrap.dataset to match initial state (required for correct toggle behavior)
  wrap.dataset.hfScopeMlx = initialMlx ? '1' : '';
  wrap.dataset.hfScopeGguf = initialGguf ? '1' : '';
  wrap.dataset.hfScopeAll = initialAll ? '1' : '';

  for (const s of scopes) {
    const btn = document.createElement('button');
    btn.type = 'button';
    btn.className = 'hf-scope-btn';
    btn.dataset.scopeKey = s.key;
    btn.textContent = s.label;
    btn.title = s.tooltip;
    btn.style.cssText =
      'padding:2px 8px;font-size:10px;font-weight:600;border:none;cursor:pointer;' +
      'color:rgba(255,255,255,0.5);background:transparent;transition:all 0.15s ease;';

    btn.addEventListener('click', () => {
      // Read current state
      let mlxActive = wrap.dataset.hfScopeMlx === '1';
      let ggufActive = wrap.dataset.hfScopeGguf === '1';
      let allActive = wrap.dataset.hfScopeAll === '1';

      if (s.key === 'all') {
        // All: toggle everything on/off (exclusive with individual toggles)
        allActive = !allActive;
        mlxActive = allActive;
        ggufActive = allActive;
      } else if (s.key === 'mlx') {
        // MLX: toggle independently
        mlxActive = !mlxActive;
        allActive = false; // deselect All if user modifies individual toggles
      } else if (s.key === 'gguf') {
        // GGUF: toggle independently
        ggufActive = !ggufActive;
        allActive = false; // deselect All if user modifies individual toggles
      }

      // Update datasets
      wrap.dataset.hfScopeMlx = mlxActive ? '1' : '';
      wrap.dataset.hfScopeGguf = ggufActive ? '1' : '';
      wrap.dataset.hfScopeAll = allActive ? '1' : '';
      container.dataset.hfScopeMlx = mlxActive ? '1' : '';
      container.dataset.hfScopeGguf = ggufActive ? '1' : '';
      container.dataset.hfScopeAll = allActive ? '1' : '';

      // Update button visuals
      wrap.querySelectorAll('.hf-scope-btn').forEach(b => {
        const isActive = b.dataset.scopeKey === 'mlx' ? mlxActive
          : b.dataset.scopeKey === 'gguf' ? ggufActive
          : b.dataset.scopeKey === 'all' ? allActive
          : false;
        setScopeBtnState(b, isActive);
      });

      // Notify caller
      if (onChange) onChange(mlxActive, ggufActive, allActive);
    });

    // Set initial state
    const active = s.key === 'all' ? initialAll
      : s.key === 'mlx' ? initialMlx
      : s.key === 'gguf' ? initialGguf
      : false;
    setScopeBtnState(btn, active);
    wrap.appendChild(btn);
  }

  container.appendChild(wrap);
  return wrap;
}

function setScopeBtnState(btn, active) {
  btn.classList.toggle('hf-scope-btn--active', active);
  btn.style.color = active ? '#fff' : 'rgba(255,255,255,0.5)';
  btn.style.background = active ? 'rgba(99,102,241,0.85)' : 'transparent';
}

// ── Sort selector (Phase 8B1) ─────────────────────────────────────────────────
// Create a sort mode selector for discovery views.
// Returns the selector element. Caller should read container.dataset.hfSearchSort.
//
// params:
//   container            – DOM element to append the selector into
//   defaultSort          – HF_SORT value (default HF_SORT.AUTO)
//   onChange             – (sort) => void  called when sort changes

export function hfCreateSortSelector({ container, defaultSort = HF_SORT.DOWNLOADS, onChange }) {
  if (!container) return null;

  const wrap = document.createElement('div');
  wrap.className = 'hf-sort-selector';
  wrap.style.cssText = 'display:flex;align-items:center;gap:4px;font-size:10px;color:rgba(255,255,255,0.5);';

  const label = document.createElement('span');
  label.textContent = 'Sort:';
  wrap.appendChild(label);

  const select = document.createElement('select');
  select.className = 'hf-sort-select';
  select.style.cssText =
    'font-size:10px;padding:2px 6px;border-radius:4px;border:1px solid rgba(255,255,255,0.1);' +
    'background:rgba(0,0,0,0.3);color:rgba(255,255,255,0.8);cursor:pointer;';

  const sorts = [
    { value: HF_SORT.AUTO, label: 'Auto (workload fit)' },
    { value: HF_SORT.RELEVANCE, label: 'Relevance' },
    { value: HF_SORT.NAME, label: 'Name' },
    { value: HF_SORT.SIZE, label: 'Size' },
    { value: HF_SORT.LAST_UPDATED, label: 'Last updated' },
  ];

  for (const s of sorts) {
    const opt = document.createElement('option');
    opt.value = s.value;
    opt.textContent = s.label;
    select.appendChild(opt);
  }

  select.value = defaultSort;
  select.addEventListener('change', () => {
    container.dataset.hfSearchSort = select.value;
    if (onChange) onChange(select.value);
  });

  container.dataset.hfSearchSort = defaultSort;
  wrap.appendChild(select);
  container.appendChild(wrap);

  return wrap;
}

// ── Format toggle chip (deprecated: use hfCreateScopeSelector) ────────────────
// Kept for backward compatibility with existing callers.

export function hfCreateFormatToggle({ container, defaultFormat = 'gguf', onChange }) {
  if (!container) return null;

  const wrap = document.createElement('div');
  wrap.className = 'hf-format-toggle';
  wrap.style.cssText = 'display:flex;gap:0;border-radius:6px;overflow:hidden;border:1px solid rgba(255,255,255,0.08);';

  const makeBtn = (fmt, label) => {
    const btn = document.createElement('button');
    btn.type = 'button';
    btn.className = `hf-format-btn${fmt === defaultFormat ? ' hf-format-btn--active' : ''}`;
    btn.dataset.format = fmt;
    btn.textContent = label;
    btn.style.cssText =
      'padding:2px 8px;font-size:10px;font-weight:600;border:none;cursor:pointer;' +
      'color:rgba(255,255,255,0.5);background:transparent;transition:all 0.15s ease;';
    btn.addEventListener('click', () => {
      wrap.querySelectorAll('.hf-format-btn').forEach(b => {
        b.classList.remove('hf-format-btn--active');
        b.style.cssText =
          b.style.cssText.replace(/color:[^;]+;|background:[^;]+;/g, '') +
          'color:rgba(255,255,255,0.5);background:transparent;';
      });
      btn.classList.add('hf-format-btn--active');
      btn.style.cssText =
        btn.style.cssText.replace(/color:[^;]+;|background:[^;]+;/g, '') +
        'color:#fff;background:rgba(99,102,241,0.85);';
      container.dataset.hfSearchFormat = fmt;
      if (onChange) onChange(fmt);
    });
    if (fmt === defaultFormat) {
      btn.style.cssText =
        btn.style.cssText.replace(/color:[^;]+;|background:[^;]+;/g, '') +
        'color:#fff;background:rgba(99,102,241,0.85);';
    }
    return btn;
  };

  wrap.appendChild(makeBtn('gguf', 'GGUF'));
  wrap.appendChild(makeBtn('mlx', 'MLX'));
  container.dataset.hfSearchFormat = defaultFormat;
  container.appendChild(wrap);

  return wrap;
}
