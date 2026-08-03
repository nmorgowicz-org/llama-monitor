// Model card panel (HF markdown README viewer) for the spawn wizard.
// Exported for use by models.js HF search panel.
import { dom } from './spawn-wizard.js';

export async function openCardPanel(repoId) {
  if (!dom.cardPanel) return;

  // Show panel + backdrop in loading state
  dom.cardPanel.classList.add('open');
  if (dom.cardBackdrop) dom.cardBackdrop.classList.add('visible');
  dom.cardPanel.setAttribute('aria-hidden', 'false');
  if (dom.cardPanelTitle) dom.cardPanelTitle.textContent = repoId;
  if (dom.cardPanelHfLink) {
    dom.cardPanelHfLink.href = `https://huggingface.co/${repoId}`;
    dom.cardPanelHfLink.textContent = '';
    const svg = dom.cardPanelHfLink.querySelector('svg') || document.createElementNS('http://www.w3.org/2000/svg','svg');
    dom.cardPanelHfLink.appendChild(svg);
    dom.cardPanelHfLink.appendChild(document.createTextNode(' huggingface.co'));
  }
  if (dom.cardLoading)    { dom.cardLoading.style.display = ''; }
  if (dom.cardError)      { dom.cardError.style.display = 'none'; dom.cardError.textContent = ''; }
  if (dom.cardFrontmatter){ dom.cardFrontmatter.style.display = 'none'; }
  if (dom.cardContent)    { dom.cardContent.style.display = 'none'; dom.cardContent.innerHTML = ''; }

  try {
    const headers = window.authHeaders ? window.authHeaders() : {};
    const resp = await fetch(`/api/hf/card?repo=${encodeURIComponent(repoId)}`, { headers });
    const data = resp.ok ? await resp.json() : { error: `HTTP ${resp.status}` };

    if (dom.cardLoading) dom.cardLoading.style.display = 'none';

    if (data.error) {
      if (dom.cardError) { dom.cardError.textContent = data.error; dom.cardError.style.display = ''; }
      return;
    }

    const raw = data.markdown || '';

    // Split off YAML front-matter (--- ... ---)
    let frontmatter = '';
    let body = raw;
    const fmMatch = raw.match(/^---\n([\s\S]*?)\n---\n?([\s\S]*)$/);
    if (fmMatch) {
      frontmatter = fmMatch[1];
      body = fmMatch[2];
    }

    if (frontmatter && dom.cardFrontmatter && dom.cardFrontmatterPre) {
      dom.cardFrontmatterPre.textContent = frontmatter;
      dom.cardFrontmatter.style.display = '';
    }

    if (dom.cardContent) {
      dom.cardContent.textContent = '';
      if (!body.trim()) {
        const p = document.createElement('p');
        p.style.cssText = 'color:var(--color-text-muted);font-size:var(--text-sm)';
        p.textContent = 'No model card content found.';
        dom.cardContent.appendChild(p);
      } else if (window.marked && window.DOMPurify) {
        // RETURN_DOM_FRAGMENT gives a sanitized DocumentFragment — no innerHTML needed
        const frag = window.DOMPurify.sanitize(window.marked.parse(body), { RETURN_DOM_FRAGMENT: true });
        dom.cardContent.appendChild(frag);
      } else {
        dom.cardContent.textContent = body;
      }
      dom.cardContent.style.display = '';
    }
  } catch (err) {
    if (dom.cardLoading) dom.cardLoading.style.display = 'none';
    if (dom.cardError) { dom.cardError.textContent = err.message || 'Failed to load model card.'; dom.cardError.style.display = ''; }
  }
}

export function _closeCardPanel() {
  if (!dom.cardPanel) return;

  // Move focus out before hiding so it's not trapped inside aria-hidden
  const wasFocused = document.activeElement;
  if (dom.cardPanel.contains(wasFocused)) {
    wasFocused.blur();
  }

  dom.cardPanel.classList.remove('open');
  dom.cardPanel.setAttribute('aria-hidden', 'true');
  if (dom.cardBackdrop) dom.cardBackdrop.classList.remove('visible');
}
