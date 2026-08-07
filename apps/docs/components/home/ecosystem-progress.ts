import type { Lang } from '@/components/home/home-content';

export type DevelopmentStage = 'building' | 'experimental' | 'preview' | 'released';

export interface ProjectProgress {
  stage: DevelopmentStage;
  value: 40 | 60 | 80 | 100;
}

const progressByProject = {
  cli: 'preview',
  code: 'released',
  web: 'preview',
  windhole: 'building',
  box: 'released',
  bench: 'preview',
  search: 'released',
  browser: 'preview',
  ocr: 'preview',
  use: 'preview',
  office: 'preview',
  science: 'preview',
  cloud: 'experimental',
  form: 'preview',
  site: 'released',
  runtime: 'preview',
  'oci-runtime': 'experimental',
  flow: 'preview',
  event: 'preview',
  lane: 'preview',
  memory: 'preview',
  orm: 'preview',
  common: 'preview',
  boot: 'preview',
  gateway: 'released',
  power: 'preview',
  ahp: 'released',
  acl: 'preview',
  tui: 'preview',
  gui: 'building',
  webview: 'preview',
  ui: 'preview',
  observer: 'preview',
  sentry: 'preview',
  updater: 'experimental',
  homebrew: 'released',
} as const satisfies Record<string, DevelopmentStage>;

const stageValues: Record<DevelopmentStage, ProjectProgress['value']> = {
  building: 40,
  experimental: 60,
  preview: 80,
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
  const stage = progressByProject[projectId as keyof typeof progressByProject];
  if (!stage) throw new Error(`Development progress is missing for ecosystem project: ${projectId}`);

  return {
    stage,
    value: stageValues[stage],
    label: stageLabels[lang][stage],
  };
}

export const progressProjectIds = Object.keys(progressByProject);
