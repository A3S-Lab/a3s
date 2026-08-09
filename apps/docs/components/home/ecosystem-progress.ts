import type { Lang } from '@/components/home/home-content';

export type DevelopmentStage = 'building' | 'experimental' | 'preview' | 'released';

export interface ProjectProgress {
  stage: DevelopmentStage;
  position: 25 | 50 | 75 | 100;
  release: string;
}

// Verified against current project versions, public releases, READMEs, and roadmaps.
// A released project can still contain explicitly preview or experimental subfeatures.
export const progressVerifiedAt = '2026-08-09';

const progressByProject = {
  cli: { stage: 'preview', release: 'v0.10.14' },
  code: { stage: 'released', release: 'v6.8.0' },
  web: { stage: 'preview', release: 'A3S v0.11.1' },
  windhole: { stage: 'preview', release: 'main' },
  box: { stage: 'released', release: 'v3.1.0' },
  bench: { stage: 'preview', release: 'v0.1.2' },
  search: { stage: 'released', release: 'v3.0.9' },
  browser: { stage: 'preview', release: 'v0.3.2' },
  ocr: { stage: 'preview', release: 'v0.5.0' },
  use: { stage: 'preview', release: 'v0.3.0' },
  office: { stage: 'preview', release: 'v0.3.0' },
  science: { stage: 'preview', release: 'main' },
  cloud: { stage: 'experimental', release: 'main' },
  form: { stage: 'preview', release: 'main' },
  site: { stage: 'released', release: 'live' },
  runtime: { stage: 'preview', release: 'v0.2.0' },
  'oci-runtime': { stage: 'experimental', release: 'v0.2.0' },
  flow: { stage: 'preview', release: 'v0.11.0' },
  event: { stage: 'preview', release: 'v0.3.0' },
  lane: { stage: 'preview', release: 'v0.5.1' },
  memory: { stage: 'preview', release: 'v0.1.2' },
  orm: { stage: 'preview', release: 'v0.2.0' },
  common: { stage: 'preview', release: 'v0.1.1' },
  boot: { stage: 'preview', release: 'v0.1.3' },
  gateway: { stage: 'released', release: 'v1.0.13' },
  power: { stage: 'preview', release: 'v0.8.0' },
  ahp: { stage: 'released', release: 'v2.4.0' },
  acl: { stage: 'preview', release: 'v0.3.0' },
  tui: { stage: 'preview', release: 'v0.1.14' },
  gui: { stage: 'preview', release: 'main' },
  webview: { stage: 'preview', release: 'v0.1.5' },
  ui: { stage: 'preview', release: 'v0.2.1' },
  observer: { stage: 'preview', release: 'v0.11.0' },
  sentry: { stage: 'preview', release: 'v0.8.0' },
  updater: { stage: 'released', release: 'v0.3.0' },
  homebrew: { stage: 'released', release: 'live' },
} as const satisfies Record<string, { stage: DevelopmentStage; release: string }>;

const stagePositions: Record<DevelopmentStage, ProjectProgress['position']> = {
  building: 25,
  experimental: 50,
  preview: 75,
  released: 100,
};

const stageLabels: Record<Lang, Record<DevelopmentStage, string>> = {
  cn: {
    building: '开发中',
    experimental: '实验阶段',
    preview: '预览阶段',
    released: '已发布',
  },
  en: {
    building: 'Building',
    experimental: 'Experimental',
    preview: 'Preview',
    released: 'Released',
  },
};

export function getProjectProgress(projectId: string, lang: Lang): ProjectProgress & { label: string } {
  const project = progressByProject[projectId as keyof typeof progressByProject];
  if (!project) throw new Error(`Delivery status is missing for ecosystem project: ${projectId}`);

  return {
    stage: project.stage,
    position: stagePositions[project.stage],
    release: project.release,
    label: stageLabels[lang][project.stage],
  };
}

export const progressProjectIds = Object.keys(progressByProject);
