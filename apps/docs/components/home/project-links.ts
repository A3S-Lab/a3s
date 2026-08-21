export interface ProjectLink {
  repository: string;
  website?: string;
}

export const projectLinks = {
  cli: { repository: 'https://github.com/A3S-Lab/Cli' },
  code: {
    repository: 'https://github.com/A3S-Lab/Code',
    website: 'https://a3s-lab.github.io/Code/',
  },
  windhole: { repository: 'https://github.com/A3S-Lab/a3s/tree/main/apps/windhole' },
  box: {
    repository: 'https://github.com/A3S-Lab/Box',
    website: 'https://a3s-lab.github.io/Box/',
  },
  bench: { repository: 'https://github.com/A3S-Lab/Bench' },
  search: { repository: 'https://github.com/A3S-Lab/Search' },
  browser: { repository: 'https://github.com/A3S-Lab/Browser' },
  ocr: { repository: 'https://github.com/A3S-Lab/OCR' },
  use: {
    repository: 'https://github.com/A3S-Lab/Use',
    website: 'https://a3s-lab.github.io/Use/',
  },
  office: {
    repository: 'https://github.com/A3S-Lab/Office',
    website: 'https://a3s-lab.github.io/Office/',
  },
  science: { repository: 'https://github.com/A3S-Lab/Science' },
  cloud: {
    repository: 'https://github.com/A3S-Lab/Cloud',
    website: 'https://a3s-lab.github.io/Cloud/',
  },
  form: {
    repository: 'https://github.com/A3S-Lab/UI',
    website: '/form/',
  },
  site: {
    repository: 'https://github.com/A3S-Lab/a3s/tree/main/apps/docs',
    website: '/',
  },
  runtime: { repository: 'https://github.com/A3S-Lab/Runtime' },
  'oci-runtime': { repository: 'https://github.com/A3S-Lab/OCI-Runtime' },
  flow: { repository: 'https://github.com/A3S-Lab/Flow' },
  event: { repository: 'https://github.com/A3S-Lab/Event' },
  lane: { repository: 'https://github.com/A3S-Lab/Lane' },
  memory: { repository: 'https://github.com/A3S-Lab/Memory' },
  orm: { repository: 'https://github.com/A3S-Lab/ORM' },
  common: { repository: 'https://github.com/A3S-Lab/a3s/tree/main/crates/common' },
  boot: { repository: 'https://github.com/A3S-Lab/Boot' },
  gateway: {
    repository: 'https://github.com/A3S-Lab/Gateway',
    website: 'https://a3s-lab.github.io/Gateway/',
  },
  power: { repository: 'https://github.com/A3S-Lab/Power' },
  ahp: { repository: 'https://github.com/A3S-Lab/AgentHarnessProtocol' },
  acl: { repository: 'https://github.com/A3S-Lab/ACL' },
  tui: { repository: 'https://github.com/A3S-Lab/TUI' },
  gui: { repository: 'https://github.com/A3S-Lab/GUI' },
  webview: { repository: 'https://github.com/A3S-Lab/WebView' },
  ui: {
    repository: 'https://github.com/A3S-Lab/UI',
    website: 'https://a3s-lab.github.io/UI/',
  },
  observer: { repository: 'https://github.com/A3S-Lab/Observer' },
  sentry: { repository: 'https://github.com/A3S-Lab/Sentry' },
  updater: { repository: 'https://github.com/A3S-Lab/a3s/tree/main/crates/updater' },
  homebrew: { repository: 'https://github.com/A3S-Lab/homebrew-tap' },
} as const satisfies Record<string, ProjectLink>;

export type ProjectId = keyof typeof projectLinks;

export function getProjectPrimaryHref(projectId: ProjectId): string {
  const project = projectLinks[projectId];
  return 'website' in project ? project.website : project.repository;
}

export function getProjectRepositoryHref(projectId: string): string {
  const project = projectLinks[projectId as ProjectId];
  if (!project) throw new Error(`Project link is missing for ecosystem project: ${projectId}`);
  return project.repository;
}

export const linkedProjectIds = Object.keys(projectLinks);
