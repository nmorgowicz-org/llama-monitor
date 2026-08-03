#!/usr/bin/env node
/**
 * Deterministic transform script: derives <version>-no_json-v21.3
 * from any froggeric/Qwen-Fixed-Chat-Templates release.
 *
 * Anchored on Jinja control-flow markers, not blind line-number edits.
 *
 * Usage:
 *   node scripts/transform-froggeric-template.mjs <input.jinja> [output.jinja]
 *
 * Self-validation (run once per install):
 *   node scripts/transform-froggeric-template.mjs --self-test --upstream <path> --reference <path>
 *   # or set env vars: FROGGERIC_V213_UPSTREAM, FROGGERIC_V213_REFERENCE
 *
 * Self-test mode requires:
 *   An upstream froggeric template and its known -no_json-v21.3 reference output.
 *   Set via --upstream/--reference flags or FROGGERIC_V213_UPSTREAM/FROGGERIC_V213_REFERENCE env vars.
 */

import { readFileSync, writeFileSync } from 'fs';
import { join } from 'path';
import { execSync } from 'child_process';

const SCRIPT_DIR = new URL('.', import.meta.url).pathname;

// Special markers used in froggeric templates:
// TOOL_CALL_OPEN: U+0107  (opening block marker)
// TOOL_CALL_END:  U+0102  (closing block marker)
// The froggeric template uses literal XML-like tags as markers:
// TOOL_CALL_OPEN = '<tool_call>' (opening block)
// TOOL_CALL_END = '</tool_call>' (closing block)
const TOOL_CALL_OPEN = '<tool_call>';
const TOOL_CALL_END = '</tool_call>';

function main() {
  const args = process.argv.slice(2);
  const parsed = parseArgs(args);

  if (parsed.selfTest) {
    return runSelfTest(parsed.upstream, parsed.reference);
  }

  if (parsed.positional.length < 1) {
    console.error('Usage: node scripts/transform-froggeric-template.mjs <input.jinja> [output.jinja]');
    console.error('       node scripts/transform-froggeric-template.mjs --self-test --upstream <path> --reference <path>');
    process.exit(1);
  }

  const inputPath = parsed.positional[0];
  let outputPath = parsed.positional[1];

  if (!outputPath) {
    const base = inputPath.replace(/\.jinja$/, '').replace(/\.jinja\.txt$/, '');
    outputPath = base + '-no_json-v21.3.jinja';
  }

  const input = readFileSync(inputPath, 'utf-8');
  const transformed = applyTransform(input);
  writeFileSync(outputPath, transformed, 'utf-8');
  console.log(`Wrote: ${outputPath}`);
}

function parseArgs(args) {
  const result = {
    upstream: null,
    reference: null,
    selfTest: false,
    positional: [],
  };
  let i = 0;
  while (i < args.length) {
    if (args[i] === '--self-test') {
      result.selfTest = true;
    } else if (args[i] === '--upstream' && args[i + 1]) {
      result.upstream = args[++i];
    } else if (args[i] === '--reference' && args[i + 1]) {
      result.reference = args[++i];
    } else {
      result.positional.push(args[i]);
    }
    i++;
  }
  return result;
}

function runSelfTest(cliUpstream, cliReference) {
  const upstreamPath = cliUpstream || process.env.FROGGERIC_V213_UPSTREAM;
  const referencePath = cliReference || process.env.FROGGERIC_V213_REFERENCE;

  if (!upstreamPath || !referencePath) {
    console.error('Error: --self-test requires --upstream and --reference flags,');
    console.error('       or FROGGERIC_V213_UPSTREAM and FROGGERIC_V213_REFERENCE env vars.');
    process.exit(1);
  }

  console.log(`Running self-test: upstream=${upstreamPath}, reference=${referencePath}`);

  const upstream = readFileSync(upstreamPath, 'utf-8');
  const reference = readFileSync(referencePath, 'utf-8');
  const transformed = applyTransform(upstream);

  if (transformed === reference) {
    console.log('PASS: Self-test diff is clean against v21.3 reference.');
    return;
  }

  console.error('FAIL: Self-test diff detected. Writing debug output...');
  const debugPath = join(SCRIPT_DIR, 'transform-debug-diff.tmp');
  writeFileSync(debugPath, transformed);
  try {
    const diff = execSync(`diff -u "${referencePath}" "${debugPath}"`, {
      encoding: 'utf-8',
      stdio: 'pipe',
    });
    console.error(diff);
  } catch (e) {
    if (e.status === 1) {
      console.error(e.stdout);
    } else {
      console.error('diff failed:', e.message);
    }
  }
  process.exit(1);
}

/**
 * Apply the -no_json-v21.3 transform anchored on Jinja control-flow markers:
 *
 * 1. Append -no_json-v21.3 to template_version
 * 2. Remove _tool_format variable declaration
 * 3. In tool_instructions block: strip all JSON-format conditional branches
 * 4. In tool-call output block: strip JSON-format branch, keep only XML
 */
function applyTransform(input) {
  let result = input;

  // 1. Append -no_json-v21.3 to template_version
  // Anchor: {%- set template_version = "..." %}
  result = result.replace(
    /\{%-\s*set\s+template_version\s*=\s*"([^"]+)"\s*%\}/,
    (match, version) => {
      const newVersion = version.endsWith('-no_json-v21.3') ? version : `${version}-no_json-v21.3`;
      return `{%- set template_version = "${newVersion}" %}`;
    },
  );

  // 2. Remove _tool_format declaration (including trailing blank line)
  // Anchor: {%- set _tool_format = ... %} followed by optional blank line
  result = result.replace(
    /^\{%-?\s*set\s+_tool_format\s*=\s*[^%]+%\}\s*\r?\n?/m,
    '',
  );

  // 3. In tool_instructions block: strip JSON-format branches
  // Anchor: {%- set tool_instructions %} ... {%- endset %}
  result = result.replace(
    /\{%-\s*set\s+tool_instructions\s*%\}([\s\S]*?)\{%-\s*endset\s*%\}/,
    (match, instructions) => {
      const cleaned = stripToolFormatJsonBranches(instructions);
      return `{%- set tool_instructions %}${cleaned}{%- endset %}`;
    },
  );

  // 4. In tool-call output block: simplify to XML-only
  // Anchor: {%- for tool_call in message.tool_calls %} ... {%- endfor %}
  // Must use depth-aware parsing because there are nested for loops inside.
  const forMatch = result.match(/\{%-\s*for\s+tool_call\s+in\s+message\.tool_calls\s+%\}/);
  if (forMatch) {
    const offset = forMatch.index + forMatch[0].length;
    const afterFor = result.substring(offset);

    // Find matching endfor by tracking depth. Track endfor position separately so body
    // does not include it (the endfor tag is the closing marker).
    let depth = 1;
    let idx = 0;
    let endforStart = -1;
    let endforTag = '';
    while (idx < afterFor.length && depth > 0) {
      if (afterFor[idx] === '{') {
        const tagMatch = afterFor.slice(idx).match(/^\{%-\s*(for|endfor)\s[^%]*%\}/);
        if (tagMatch) {
          if (tagMatch[1] === 'for') depth++;
          else {
            depth--;
            if (depth === 0) {
              endforStart = idx;
              endforTag = tagMatch[0];
            }
          }
          idx += tagMatch[0].length;
          continue;
        }
      }
      idx++;
    }

    if (endforStart !== -1 && endforTag) {
      const body = afterFor.substring(0, endforStart);
      const afterEndfor = afterFor.substring(idx);
      const cleaned = simplifyToolCallOutput(body);
      result = result.substring(0, forMatch.index) +
        `{%- for tool_call in message.tool_calls %}${cleaned}${endforTag}` +
        afterEndfor;
    }
  }

  return result;
}

/**
 * Strip JSON-format conditional branches from the tool_instructions block.
 *
 * Pattern: {%- if _tool_format == 'json' %}A{%- else %}B{%- endif %}  ->  B
 * Pattern: {%- if _tool_format == 'json' %}A{%- endif %}             ->  (removed)
 *
 * Uses depth-tracked parsing of Jinja conditionals.
 */
function stripToolFormatJsonBranches(block) {
  const lines = block.split('\n');
  const result = [];
  let i = 0;

  while (i < lines.length) {
    const line = lines[i];

    // Check for {%- if _tool_format == 'json' %}
    if (/^\s*\{%-\s*if\s+_tool_format\s*===?\s*'json'\s*%\}/.test(line)) {
      // Collect all lines until matching endif
      i++;
      const segmentLines = [];
      let depth = 1;
      while (i < lines.length && depth > 0) {
        segmentLines.push(lines[i]);
        if (/^\s*\{%-\s*if/.test(lines[i])) depth++;
        if (/^\s*\{%-\s*endif/.test(lines[i])) depth--;
        i++;
      }

      // Find else branch by scanning forward through segmentLines.
      // ScanDepth starts at 1 because we're already inside the outer if-block.
      let elseIdx = -1;
      let endIdx = -1;
      let scanDepth = 1;
      for (let j = 0; j < segmentLines.length; j++) {
        if (/^\s*\{%-\s*if/.test(segmentLines[j])) {
          scanDepth++;
        } else if (/^\s*\{%-\s*endif/.test(segmentLines[j])) {
          scanDepth--;
          if (scanDepth === 0) {
            endIdx = j;
            break;
          }
        } else if (/^\s*\{%-\s*else/.test(segmentLines[j]) && scanDepth === 1) {
          elseIdx = j;
        }
      }

      if (elseIdx !== -1 && endIdx !== -1) {
        // Keep else branch content (between else and endif, exclusive of endif)
        for (let j = elseIdx + 1; j < endIdx; j++) {
          result.push(segmentLines[j]);
        }
      }
      // else: no else branch, remove the entire if block
    } else {
      result.push(line);
      i++;
    }
  }

  return result.join('\n');
}

/**
 * Simplify the tool-call output block by removing the JSON branch.
 *
 * Strategy: parse Jinja conditionals, extract the XML branch from
 * {%- if _tool_format == 'json' %}...{%- else %}XML...{%- endif %},
 * then dedent and add loop.last handling to the closing tag.
 */
function simplifyToolCallOutput(block) {
  const lines = block.split('\n');
  const result = [];
  let i = 0;

  while (i < lines.length) {
    const line = lines[i];

    // Check for {%- if _tool_format == 'json' %}
    if (/^\s*\{%-\s*if\s+_tool_format\s*===?\s*'json'\s*%\}/.test(line)) {
      // Collect all lines until matching endif
      i++;
      const segmentLines = [];
      let depth = 1;
      while (i < lines.length && depth > 0) {
        segmentLines.push(lines[i]);
        if (/^\s*\{%-\s*if/.test(lines[i])) depth++;
        if (/^\s*\{%-\s*endif/.test(lines[i])) depth--;
        i++;
      }

      // Find else branch by scanning forward through segmentLines.
      // ScanDepth starts at 1 because we're already inside the outer if-block.
      let elseIdx = -1;
      let endIdx = -1;
      let scanDepth = 1;
      for (let j = 0; j < segmentLines.length; j++) {
        if (/^\s*\{%-\s*if/.test(segmentLines[j])) {
          scanDepth++;
        } else if (/^\s*\{%-\s*endif/.test(segmentLines[j])) {
          scanDepth--;
          if (scanDepth === 0) {
            endIdx = j;
            break;
          }
        } else if (/^\s*\{%-\s*else/.test(segmentLines[j]) && scanDepth === 1) {
          elseIdx = j;
        }
      }

      if (elseIdx !== -1 && endIdx !== -1) {
        // Extract else branch content (exclusive of endif) and dedent by one level (4 spaces)
        const elseLines = segmentLines.slice(elseIdx + 1, endIdx);
        for (const l of elseLines) {
          if (/^    /m.test(l)) {
            result.push(l.slice(4));
          } else {
            result.push(l);
          }
        }
      }
    } else {
      result.push(line);
      i++;
    }
  }

  // Post-process: add loop.last handling to the bare closing tag.
  // Upstream has: {{- '</function>\n_${TOOL_CALL_END}' }}
  // Reference has: {%- if loop.last %}{{- '</function>\n_${TOOL_CALL_END}\n' }}{%- else %}{{- '</function>\n_${TOOL_CALL_END}' }}{%- endif %}

  const closingTag = '</function>\\n' + TOOL_CALL_END;
  const closingTagWithNewline = '</function>\\n' + TOOL_CALL_END + '\\n';

  const fixed = result.map((line) => {
    // Match bare closing tag line (not already in a loop.last conditional)
    if (
      line.includes(closingTag) &&
      !line.includes(closingTagWithNewline) &&
      !line.includes('loop.last') &&
      !line.includes('{%- else')
    ) {
      const indent = line.match(/^(\s*)/)[1];
      return (
        indent +
        '{%- if loop.last %}\n' +
        indent +
        "    {{- '" +
        closingTagWithNewline +
        "' }}\n" +
        indent +
        '{%- else %}\n' +
        indent +
        "    {{- '" +
        closingTag +
        "' }}\n" +
        indent +
        '{%- endif %}'
      );
    }
    return line;
  }).join('\n');

  return fixed;
}

if (process.argv[1] === join(SCRIPT_DIR, 'transform-froggeric-template.mjs')) {
  main();
}
