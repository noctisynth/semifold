import { mkdir, readFile, readdir, writeFile } from 'node:fs/promises';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

export const generatedStart = '# BEGIN GENERATED SKILL GUIDES';
export const generatedEnd = '# END GENERATED SKILL GUIDES';
const languages = ['en', 'zh'];
const taskPattern = /^[a-z][a-z0-9-]*$/;
const commandPattern = /^[a-z][a-z0-9-]*(?: [a-z][a-z0-9-]*)*$/;

function normalize(source) {
  return source.replace(/\r\n/g, '\n');
}

function requireText(value, field, path) {
  if (typeof value !== 'string' || !value.trim() || /[\r\n]/.test(value)) {
    throw new Error(`${path}: ${field} must be a nonempty single-line string.`);
  }
  return value.trim();
}

function plainMarkdown(body, path) {
  let fence;
  const prose = body.split('\n').map((line) => {
    const marker = line.match(/^\s{0,3}(`{3,}|~{3,})(.*)$/);
    if (marker) {
      if (!fence) fence = marker[1];
      else if (marker[1][0] === fence[0] && marker[1].length >= fence.length && !marker[2].trim()) {
        fence = undefined;
      }
      return '';
    }
    return fence ? '' : line;
  }).join('\n').replace(/(`+)[^\n]*?\1/g, '');
  if (fence) throw new Error(`${path}: Markdown code fences must be closed.`);
  if (/^\s*(?:import|export)\s/m.test(prose) || /(?<!\\)[{}]/.test(prose) ||
      /<\/?[A-Za-z][\w.-]*(?:\s|\/?>)|<>|<\/?>|<!--|<![A-Z]/.test(prose)) {
    throw new Error(`${path}: task guides support plain Markdown, not MDX expressions, imports, or HTML/JSX.`);
  }
  const links = [
    ...prose.matchAll(/!?\[[^\]]*\]\(([^\s)]+)(?:\s+"[^"]*")?\)/g),
    ...prose.matchAll(/^\s*\[[^\]]+\]:\s*(\S+)/gm),
  ];
  for (const [, destination] of links) {
    const target = destination.replace(/^<|>$/g, '');
    if (!/^https?:\/\//.test(target) && !target.startsWith('#')) {
      throw new Error(`${path}: use an absolute HTTP(S) documentation link, got ${destination}.`);
    }
  }
  return body;
}

export function parseTask(source, path) {
  const match = normalize(source).match(/^---\n([\s\S]*?)\n---(?:\n|$)([\s\S]*)$/);
  if (!match) throw new Error(`${path}: task guide needs YAML frontmatter.`);
  const metadata = Bun.YAML.parse(match[1]);
  if (!metadata || typeof metadata !== 'object' || Array.isArray(metadata)) {
    throw new Error(`${path}: frontmatter must be an object.`);
  }
  const task = requireText(metadata.skillTask, 'skillTask', path);
  if (!taskPattern.test(task) || !path.endsWith(`/${task}.mdx`)) {
    throw new Error(`${path}: skillTask must be a stable lowercase ID matching the file name.`);
  }
  const title = requireText(metadata.title, 'title', path);
  const summary = requireText(metadata.description, 'description', path);
  const commands = metadata.skillCommands;
  if (!Array.isArray(commands) || commands.some((command) => typeof command !== 'string' || !commandPattern.test(command)) ||
      new Set(commands).size !== commands.length || (task === 'shared' ? commands.length !== 0 : commands.length === 0)) {
    throw new Error(`${path}: skillCommands must contain unique command paths; only shared has no commands.`);
  }
  const body = match[2].trim();
  if (!body) throw new Error(`${path}: task guide body must not be empty.`);
  plainMarkdown(body, path);
  return { task, summary, commands, instructions: `# ${title}\n\n${body}` };
}

export function replaceGeneratedGuides(locale, block, path) {
  const source = normalize(locale);
  const starts = source.split(generatedStart).length - 1;
  const ends = source.split(generatedEnd).length - 1;
  let prefix = source;
  if (starts || ends) {
    if (starts !== 1 || ends !== 1 || source.indexOf(generatedEnd) < source.indexOf(generatedStart) ||
        source.slice(source.indexOf(generatedEnd) + generatedEnd.length).trim()) {
      throw new Error(`${path}: expected one complete generated guide block at the end of the locale file.`);
    }
    prefix = source.slice(0, source.indexOf(generatedStart));
  }
  if (/^\[cli\.skill\.guides\]\s*$/m.test(prefix)) {
    throw new Error(`${path}: cli.skill.guides must exist only inside the generated block.`);
  }
  return `${prefix.trimEnd()}\n\n${block}\n`;
}

/** Render every artifact from caller-supplied sources without accessing the filesystem. */
export function generateArtifacts({ documents, skill, locales }) {
  const guides = new Map(languages.map((language) => [language, new Map()]));
  for (const document of documents) {
    const language = guides.get(document.language);
    if (!language) throw new Error(`Unsupported task guide language: ${document.language}.`);
    const guide = parseTask(document.source, document.path);
    if (language.has(guide.task)) throw new Error(`${document.path}: duplicate task ID ${guide.task}.`);
    language.set(guide.task, guide);
  }
  const ids = [...guides.get('en').keys()].sort();
  if (!ids.includes('shared') || ids.length < 2) throw new Error('Task guides need shared rules and at least one task.');
  if (JSON.stringify(ids) !== JSON.stringify([...guides.get('zh').keys()].sort())) {
    throw new Error('English and Chinese skill task IDs must match.');
  }
  for (const task of ids) {
    if (JSON.stringify(guides.get('en').get(task).commands) !== JSON.stringify(guides.get('zh').get(task).commands)) {
      throw new Error(`Task ${task}: English and Chinese command paths must match.`);
    }
  }
  const tasks = ids.filter((task) => task !== 'shared');
  const artifacts = new Map([
    ['crates/semifold/assets/skill/tasks.json', `${JSON.stringify(tasks.map((task) => ({
      task, commands: guides.get('en').get(task).commands,
    })), null, 2)}\n`],
    ['crates/semifold/assets/skill/SKILL.md', normalize(skill)],
  ]);
  for (const language of languages) {
    const lines = [generatedStart, '# Generated by docs/scripts/generate-skill.mjs from docs/content/docs; do not edit.',
      '[cli.skill.guides]', `shared = ${JSON.stringify(guides.get(language).get('shared').instructions)}`];
    for (const task of tasks) {
      const guide = guides.get(language).get(task);
      lines.push(`${task} = ${JSON.stringify(guide.instructions)}`, `${task}_summary = ${JSON.stringify(guide.summary)}`);
    }
    lines.push(generatedEnd);
    const path = `crates/semifold/locales/${language}.toml`;
    if (typeof locales[language] !== 'string') throw new Error(`Missing locale source: ${path}.`);
    artifacts.set(path, replaceGeneratedGuides(locales[language], lines.join('\n'), path));
  }
  return artifacts;
}

export async function readSources(root) {
  const documents = [];
  const locales = {};
  for (const language of languages) {
    const folder = `docs/content/docs/${language}/agent-tasks`;
    for (const file of (await readdir(resolve(root, folder))).filter((name) => name.endsWith('.mdx')).sort()) {
      const path = `${folder}/${file}`;
      documents.push({ language, path, source: await readFile(resolve(root, path), 'utf8') });
    }
    locales[language] = await readFile(resolve(root, `crates/semifold/locales/${language}.toml`), 'utf8');
  }
  const skill = await readFile(resolve(root, 'skills/semifold/SKILL.md'), 'utf8');
  return { documents, skill, locales };
}

export async function findDrift(root, artifacts) {
  const drift = [];
  for (const [path, expected] of artifacts) {
    let actual;
    try {
      actual = await readFile(resolve(root, path), 'utf8');
    } catch (error) {
      if (error.code !== 'ENOENT') throw error;
    }
    if (actual === undefined || normalize(actual) !== expected) drift.push(path);
  }
  return drift;
}

if (import.meta.main) {
  const args = process.argv.slice(2);
  if (args.length > 1 || args.some((arg) => arg !== '--check')) {
    throw new Error('Usage: bun scripts/generate-skill.mjs [--check]');
  }
  const root = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
  const artifacts = generateArtifacts(await readSources(root));
  const drift = await findDrift(root, artifacts);
  if (args.includes('--check')) {
    if (drift.length) throw new Error(`Generated skill artifacts are out of date:\n${drift.join('\n')}\nRun bun run generate:skill in docs/.`);
    console.log('Skill generation: checked-in task index, bootstrap, and bilingual guides match their sources.');
  } else {
    for (const path of drift) {
      await mkdir(dirname(resolve(root, path)), { recursive: true });
      await writeFile(resolve(root, path), artifacts.get(path));
    }
    console.log(`Skill generation: updated ${drift.length} artifact(s).`);
  }
}
