export const COMMUNITY_TEMPLATES = {
  qwen: {
    name: 'qwen-froggeric-fixed',
    display: "froggeric's Fixed Template",
    installEndpoint: '/api/chat-template/install-hf',
    repo: 'froggeric/Qwen-Fixed-Chat-Templates',
    file: 'chat_template.jinja',
    description: 'Fixes tool calling, KV cache invalidation & agentic loop bugs for Qwen 3.5 / 3.6',
    sourceUrl: 'https://huggingface.co/froggeric/Qwen-Fixed-Chat-Templates',
    provenance: 'community',
    transformed: true, // Uses -no_json transform at install time
  },
  // Google's official template is listed first (and is what getDefaultTemplateForFamily()
  // picks) because it's the priority recommendation. jscott3201's agentic fork is kept as
  // a fallback entry in case Google's template regresses tool-calling again in the future.
  gemma4: [
    {
      name: 'gemma4-google-official',
      display: "Google's Official Gemma 4 Template",
      installEndpoint: '/api/chat-template/install-hf',
      repo: 'google/gemma-4-31B-it',
      file: 'chat_template.jinja',
      description: 'Reference template shipped with Gemma 4 31B (from model repo)',
      sourceUrl: 'https://huggingface.co/google/gemma-4-31B-it/blob/main/chat_template.jinja',
      provenance: 'official',
    },
    {
      name: 'gemma4-jscott3201-agentic',
      display: "jscott3201's Gemma 4 Agentic Template",
      installEndpoint: '/api/chat-template/install-url',
      url: 'https://raw.githubusercontent.com/jscott3201/llm-tuning/main/gemma4/chat_templates/custom_pub_chat_template_gemma4.jinja',
      description: 'Improves thinking, tool calls, null arguments & multi-turn agentic workflows for Gemma 4',
      sourceUrl: 'https://github.com/jscott3201/llm-tuning/blob/main/gemma4/chat_templates/custom_pub_chat_template_gemma4.jinja',
      provenance: 'community',
    },
  ],
};

/** Returns array of templates for a family (normalizes single object → array). */
export function getTemplatesForFamily(family) {
  if (!family) return [];
  const entry = COMMUNITY_TEMPLATES[family];
  if (!entry) return [];
  return Array.isArray(entry) ? entry : [entry];
}

/** Returns the default template for a family (first candidate). */
export function getDefaultTemplateForFamily(family) {
  return getTemplatesForFamily(family)[0] || null;
}

/** Returns all family keys that have templates. */
export function getTemplateFamilies() {
  return Object.keys(COMMUNITY_TEMPLATES);
}

// Maps a backend-derived architecture family slug (e.g. preset.family /
// wizardState.model.family — sourced from GGUF `general.architecture` or an
// HF `base_model` tag, never a filename) to a community template group.
export function communityTemplateFamilyFor(family) {
  const f = (family || '').toLowerCase();
  if (!f) return null;
  if (f.startsWith('qwen') || f === 'qwopus') return 'qwen';
  if (f === 'gemma4') return 'gemma4';
  return null;
}

// Maps a raw GGUF `general.architecture` value (e.g. "qwen3_6", "qwen35moe")
// directly to a community template group. Used when no normalized family
// slug is available yet but live GGUF metadata was just read.
export function communityFamilyFromGgufArchitecture(arch) {
  const a = (arch || '').toLowerCase();
  if (a.includes('qwen')) return 'qwen';
  if (a.includes('gemma4') || a.includes('gemma_4')) return 'gemma4';
  return null;
}

export function buildCommunityTemplateInstallRequest(template, force = false) {
  const body = template.url
    ? { url: template.url, name: template.name }
    : { repo: template.repo, file: template.file, name: template.name };
  if (force) body.force = true;
  return { endpoint: template.installEndpoint, body };
}
