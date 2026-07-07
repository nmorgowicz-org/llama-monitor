export const COMMUNITY_TEMPLATES = {
  qwen: {
    name: 'qwen-froggeric-fixed',
    display: "froggeric's Fixed Template",
    installEndpoint: '/api/chat-template/install-hf',
    repo: 'froggeric/Qwen-Fixed-Chat-Templates',
    file: 'chat_template.jinja',
    description: 'Fixes tool calling, KV cache invalidation & agentic loop bugs for Qwen 3.5 / 3.6',
    sourceUrl: 'https://huggingface.co/froggeric/Qwen-Fixed-Chat-Templates',
  },
  gemma4: {
    name: 'gemma4-jscott3201-agentic',
    display: "jscott3201's Gemma 4 Agentic Template",
    installEndpoint: '/api/chat-template/install-url',
    url: 'https://raw.githubusercontent.com/jscott3201/llm-tuning/main/gemma4/chat_templates/custom_pub_chat_template_gemma4.jinja',
    description: 'Improves thinking, tool calls, null arguments & multi-turn agentic workflows for Gemma 4',
    sourceUrl: 'https://github.com/jscott3201/llm-tuning/blob/main/gemma4/chat_templates/custom_pub_chat_template_gemma4.jinja',
  },
};

export function detectCommunityTemplateFamily(name) {
  const lower = (name || '').toLowerCase();
  if (lower.includes('qwen') || lower.includes('qwopus')) return 'qwen';
  if (lower.includes('gemma-4') || lower.includes('gemma4')) return 'gemma4';
  return null;
}

export function buildCommunityTemplateInstallRequest(template, force = false) {
  const body = template.url
    ? { url: template.url, name: template.name }
    : { repo: template.repo, file: template.file, name: template.name };
  if (force) body.force = true;
  return { endpoint: template.installEndpoint, body };
}
