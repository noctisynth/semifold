import { describe, expect, test } from 'bun:test';
import { existsSync } from 'node:fs';
import { mkdtemp, mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, resolve } from 'node:path';
import { findDrift, generateArtifacts, generatedEnd, generatedStart, parseTask, readSources, replaceGeneratedGuides } from './generate-skill.mjs';

function document(language, task, commands = task === 'shared' ? [] : [task], body = 'Read the repository instructions.') {
  return {
    language, path: `${language}/${task}.mdx`,
    source: `---\ntitle: ${language} ${task}\ndescription: Guide for ${task}.\nskillTask: ${task}\nskillCommands: ${JSON.stringify(commands)}\n---\n\n${body}\n`,
  };
}

function sources() {
  return {
    documents: ['en', 'zh'].flatMap((language) => [document(language, 'shared'), document(language, 'init')]),
    skill: '---\nname: semifold\ndescription: Discover local release task guides.\n---\n\n# Semifold\n',
    locales: { en: '[cli.commands]\nskill = "Read guides"\n', zh: '[cli.commands]\nskill = "读取指南"\n' },
  };
}

describe('skill generator', () => {
  test('renders a stable task index and localized TOML that round-trips the Markdown', () => {
    const input = sources();
    input.documents[1] = document('en', 'init', ['init'], 'Use `smif init`.\n\n"Quoted" \\ path and 中文.');
    const artifacts = generateArtifacts(input);
    expect(JSON.parse(artifacts.get('crates/semifold/assets/skill/tasks.json'))).toEqual([{ task: 'init', commands: ['init'] }]);
    expect(artifacts.get('crates/semifold/assets/skill/SKILL.md')).toBe(input.skill);
    const english = Bun.TOML.parse(artifacts.get('crates/semifold/locales/en.toml'));
    expect(english.cli.commands.skill).toBe('Read guides');
    expect(english.cli.skill.guides.init).toBe('# en init\n\nUse `smif init`.\n\n"Quoted" \\ path and 中文.');
    expect(english.cli.skill.guides.init_summary).toBe('Guide for init.');
    const chinese = Bun.TOML.parse(artifacts.get('crates/semifold/locales/zh.toml'));
    expect(Object.keys(english.cli.skill.guides)).toEqual(Object.keys(chinese.cli.skill.guides));
    expect(generateArtifacts({ ...input, documents: [...input.documents].reverse() })).toEqual(artifacts);
  });

  test('rejects missing translations, command mismatches, duplicate IDs, and invalid metadata', () => {
    const input = sources();
    expect(() => generateArtifacts({ ...input, documents: input.documents.slice(0, -1) })).toThrow('IDs must match');
    const mismatch = [...input.documents];
    mismatch[3] = document('zh', 'init', ['config']);
    expect(() => generateArtifacts({ ...input, documents: mismatch })).toThrow('command paths must match');
    expect(() => generateArtifacts({ ...input, documents: [...input.documents, input.documents[0]] })).toThrow('duplicate task ID');
    for (const commands of [[], [''], ['init --help'], ['init', 'init']]) {
      const invalid = document('en', 'init', commands);
      expect(() => parseTask(invalid.source, invalid.path)).toThrow('skillCommands');
    }
    const blank = document('en', 'init', ['init'], '');
    expect(() => parseTask(blank.source, blank.path)).toThrow('body must not be empty');
    expect(() => parseTask(blank.source.replace('description: Guide for init.', 'description: ""'), blank.path)).toThrow('description');
    expect(() => parseTask(blank.source, 'en/other.mdx')).toThrow('matching the file name');
  });

  test('rejects unsupported MDX and relative links but preserves literal examples', () => {
    for (const body of ['<Callout>Body</Callout>', '{dynamicValue}', 'import Guide from "./guide";',
      'export const value = 1;', '<>fragment</>', '[Read](../init/)', '[Read][ref]\n\n[ref]: /docs/commands/init/', '```bash\nsmif init']) {
      const invalid = document('en', 'init', ['init'], body);
      expect(() => parseTask(invalid.source, invalid.path)).toThrow();
    }
    const body = '[Read](https://semifold.noctisynth.org/docs/commands/init/) and `[literal](../init/)`\n\n```md\n<Component value={1} />\n```';
    const valid = document('en', 'init', ['init'], body);
    expect(parseTask(valid.source, valid.path).instructions).toEndWith(body);
  });

  test('updates only the generated tail and detects malformed markers', () => {
    const original = '# Handwritten comment\n[cli.commands]\nskill = "Guides"\n';
    const old = `${generatedStart}\n[cli.skill.guides]\nshared = "old"\n${generatedEnd}`;
    const next = `${generatedStart}\n[cli.skill.guides]\nshared = "new"\n${generatedEnd}`;
    const updated = replaceGeneratedGuides(`${original}\n${old}\n`, next, 'en.toml');
    expect(updated).toBe(`${original}\n${next}\n`);
    expect(replaceGeneratedGuides(updated, next, 'en.toml')).toBe(updated);
    for (const broken of [generatedStart, generatedEnd, `${old}\n${old}`, `${old}\n[manual]\nvalue = 1`]) {
      expect(() => replaceGeneratedGuides(broken, next, 'en.toml')).toThrow('generated guide block');
    }
  });

  test('drift inspection detects changed or missing output without writing', async () => {
    const root = await mkdtemp(resolve(tmpdir(), 'semifold-skill-generation-'));
    try {
      const artifacts = generateArtifacts(sources());
      const changed = 'crates/semifold/assets/skill/tasks.json';
      for (const [path, content] of artifacts) {
        await mkdir(dirname(resolve(root, path)), { recursive: true });
        await writeFile(resolve(root, path), path === changed ? 'stale\n' : content);
      }
      expect(await findDrift(root, artifacts)).toEqual([changed]);
      expect(await readFile(resolve(root, changed), 'utf8')).toBe('stale\n');
      await rm(resolve(root, changed));
      expect(await findDrift(root, artifacts)).toEqual([changed]);
    } finally {
      await rm(root, { recursive: true, force: true });
    }
  });
});

test('checked-in skill artifacts match the maintained bilingual task sources', async () => {
  const root = resolve(import.meta.dir, '../..');
  const input = await readSources(root);
  const artifacts = generateArtifacts(input);
  expect(await findDrift(root, artifacts)).toEqual([]);
  const tasks = JSON.parse(artifacts.get('crates/semifold/assets/skill/tasks.json'));
  expect(tasks.map(({ task }) => task)).toEqual(['changeset', 'config', 'init', 'release', 'status']);
});

test('task guide product links resolve to maintained pages in the matching language', async () => {
  const root = resolve(import.meta.dir, '../..');
  const { documents } = await readSources(root);
  for (const { source, language } of documents) {
    for (const [, chinese, slug] of source.matchAll(/https:\/\/semifold\.noctisynth\.org\/(zh\/)?docs\/([^\s)]+)\//g)) {
      expect(chinese ? 'zh' : 'en').toBe(language);
      expect(existsSync(resolve(root, `docs/content/docs/${language}/${slug}.mdx`))).toBe(true);
    }
  }
});
