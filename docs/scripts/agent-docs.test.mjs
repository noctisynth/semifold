import { describe, expect, test } from 'bun:test';
import { readFileSync, readdirSync } from 'node:fs';
import { resolve } from 'node:path';
import { markdownPath, renderIndex, renderMarkdown } from '../lib/agent-docs.ts';

const page = {
  slugs: ['commands', 'ci'], url: '/docs/commands/ci', locale: 'en',
  data: { title: 'CI', description: 'Prepare a release PR.' },
};
const chinese = { ...page, locale: 'zh', url: '/zh/docs/commands/ci', data: { title: '持续集成' } };

describe('agent documentation projections', () => {
  test('each language indexes only its own pages with one document title', () => {
    const en = renderIndex([chinese, page], 'en');
    const zh = renderIndex([page, chinese], 'zh');
    expect(en.match(/^# /gm)).toHaveLength(1);
    expect(zh.match(/^# /gm)).toHaveLength(1);
    expect(en).toContain('/markdown/en/commands/ci.md');
    expect(en).not.toContain('/markdown/zh/commands/ci.md');
    expect(zh).toContain('/markdown/zh/commands/ci.md');
    expect(zh).not.toContain('/markdown/en/commands/ci.md');
    expect(en).toContain('/zh/llms.txt');
    expect(zh).toContain('/llms.txt');
  });

  test('root pages have stable file names and page metadata survives', () => {
    expect(markdownPath({ ...page, slugs: [] })).toBe('/markdown/en/index.md');
    expect(markdownPath({ ...chinese, slugs: [] })).toBe('/markdown/zh/index.md');
    const content = renderMarkdown(chinese, '正文。');
    expect(content).toContain('# 持续集成');
    expect(content).toContain('Language: zh');
    expect(content).toContain('Source: https://semifold.noctisynth.org/zh/docs/commands/ci/');
    expect(content).toContain('正文。');
  });

  test('prose links retain their original targets while code examples stay literal', () => {
    const body = [
      '[Status](../status/) and [local](#recovery)',
      '`[Literal](../status/)`',
      '[Config](/docs/configuration/reference/) and [external](https://example.org/)',
      '```md', '[Example](../status/)', '```',
      '~~~md', '[Example](/docs/commands/ci/)', '~~~',
    ].join('\n');
    const result = renderMarkdown(page, body);
    expect(result).toContain('[Status](https://semifold.noctisynth.org/docs/commands/status/)');
    expect(result).toContain('[local](#recovery)');
    expect(result).toContain('`[Literal](../status/)`');
    expect(result).toContain('[Config](https://semifold.noctisynth.org/docs/configuration/reference/)');
    expect(result).toContain('[external](https://example.org/)');
    expect(result).toContain('```md\n[Example](../status/)\n```');
    expect(result).toContain('~~~md\n[Example](/docs/commands/ci/)\n~~~');
  });

  test('index output does not depend on source discovery order', () => {
    const another = { ...page, slugs: ['configuration', 'reference'], url: '/docs/configuration/reference' };
    expect(renderIndex([page, another], 'en')).toBe(renderIndex([another, page], 'en'));
  });
});

test('distributed skill has valid discovery metadata and is a self-contained entrypoint', () => {
  const root = resolve(import.meta.dir, '../../skills/semifold');
  const skill = readFileSync(resolve(root, 'SKILL.md'), 'utf8');
  const frontmatter = Bun.YAML.parse(skill.split('---')[1]);
  expect(frontmatter.name).toBe('semifold');
  expect(frontmatter.description.length).toBeGreaterThan(0);
  expect(frontmatter.description.length).toBeLessThanOrEqual(1024);
  expect([...skill.matchAll(/\]\((references\/[^)]+)\)/g)]).toHaveLength(0);
  expect(readdirSync(root)).toEqual(['SKILL.md']);
  expect(skill).not.toContain('[TODO:');
});
