/** Pure projections of the public documentation source for agent readers. */
export const documentationOrigin = 'https://semifold.noctisynth.org';
export const skillUrl = 'https://github.com/noctisynth/semifold/tree/main/skills/semifold';

export interface AgentPage {
  slugs: string[];
  url: string;
  locale?: string;
  data: { title: string; description?: string };
}

export function markdownPath(page: AgentPage): string {
  return `/markdown/${page.locale ?? 'en'}/${page.slugs.join('/') || 'index'}.md`;
}

export function indexPath(locale: string): string {
  return locale === 'zh' ? '/zh/llms.txt' : '/llms.txt';
}

function singleLine(text: string): string {
  return text.replace(/[\r\n]+/g, ' ').replace(/[[\]]/g, '').trim();
}

export function renderIndex(pages: AgentPage[], locale: 'en' | 'zh'): string {
  const chinese = locale === 'zh';
  const lines = [
    chinese ? '# Semifold 中文文档' : '# Semifold documentation', '',
    chinese ? '> 跨语言单仓库的版本、变更日志与发布管理。' : '> Version, changelog, and release management for cross-language monorepos.', '',
    chinese ? '`smif skill list` 列出可用任务，`smif skill get <task>` 返回当前 CLI 版本的任务指南。以下为产品文档的 Markdown 页面。' : '`smif skill list` lists available tasks; `smif skill get <task>` returns guidance for the installed CLI version. The Markdown pages below provide product documentation.', '',
  ];
  const groups = new Map<string, AgentPage[]>();
  for (const page of pages.filter((page) => (page.locale ?? 'en') === locale)
    .sort((a, b) => a.url.localeCompare(b.url))) {
    const group = page.slugs.length > 1 ? page.slugs[0] : 'overview';
    const entries = groups.get(group) ?? [];
    entries.push(page);
    groups.set(group, entries);
  }
  const labels: Record<string, [string, string]> = {
    overview: ['Overview', '概览'], 'getting-started': ['Getting started', '开始使用'],
    'agent-tasks': ['Agent task guides', 'Agent 任务指南'],
    commands: ['Commands', '命令'], configuration: ['Configuration', '配置'],
    workspace: ['Workspaces', '工作区'], concepts: ['Concepts', '概念'], plugins: ['Plugins', '插件'],
  };
  for (const [group, entries] of groups) {
    lines.push(`## ${labels[group]?.[chinese ? 1 : 0] ?? group}`, '');
    for (const page of entries) {
      lines.push(`- [${singleLine(page.data.title)}](${documentationOrigin}${markdownPath(page)}): ${singleLine(page.data.description ?? '')}`);
    }
    lines.push('');
  }
  lines.push(chinese ? '## 更多入口' : '## Additional resources', '',
    `- [${chinese ? '英文索引' : '中文索引'}](${documentationOrigin}${indexPath(chinese ? 'en' : 'zh')})`,
    `- [${chinese ? '双语全文' : 'Complete bilingual documentation'}](${documentationOrigin}/llms-full.txt)`,
    `- [Semifold Skill](${skillUrl})`, '');
  return lines.join('\n');
}

/** Resolve prose links against the HTML source while preserving fenced examples. */
export function renderMarkdown(page: AgentPage, body: string): string {
  const sourceUrl = new URL(page.url.endsWith('/') ? page.url : `${page.url}/`, documentationOrigin);
  let fence: string | undefined;
  const content = body.split('\n').map((line) => {
    const marker = line.match(/^\s{0,3}(`{3,}|~{3,})/);
    if (marker) {
      if (!fence) fence = marker[1];
      else if (marker[1][0] === fence[0] && marker[1].length >= fence.length) fence = undefined;
      return line;
    }
    if (fence) return line;
    return line.replace(/(`+).*?\1|(!?\[[^\]]*\]\()([^\s)]+)(\))/g, (original, code, prefix, href, suffix) => {
      if (code || href.startsWith('#') || /^[a-z][a-z0-9+.-]*:/i.test(href)) return original;
      return `${prefix}${new URL(href, sourceUrl).href}${suffix}`;
    });
  }).join('\n');
  return [`# ${page.data.title}`, '', page.data.description ?? '', '',
    `Source: ${sourceUrl.href}`, `Language: ${page.locale ?? 'en'}`, '', content, ''].join('\n');
}
