import type { Lang } from '@/components/home/home-content';

export const deliveryStages = ['building', 'experimental', 'preview', 'released'] as const;

export type DeliveryStage = (typeof deliveryStages)[number];

export interface ProjectDeliveryStatus {
  stage: DeliveryStage;
  release: string;
}

interface DeliveryStageCopy {
  label: string;
  description: string;
}

// Verified against current project versions, public releases, READMEs, and roadmaps.
// A released project can still contain explicitly preview or experimental subfeatures.
export const statusVerifiedAt = '2026-09-05';

const statusByProject = {
  cli: { stage: 'preview', release: 'v0.12.5' },
  code: { stage: 'released', release: 'v8.2.0' },
  desktop: { stage: 'preview', release: 'v0.1.0' },
  windhole: { stage: 'preview', release: 'main' },
  box: { stage: 'released', release: 'v3.2.3' },
  bench: { stage: 'preview', release: 'v0.1.2' },
  search: { stage: 'released', release: 'v3.1.1' },
  browser: { stage: 'preview', release: 'v0.3.2' },
  ocr: { stage: 'preview', release: 'v0.5.0' },
  use: { stage: 'preview', release: 'v0.3.2' },
  office: { stage: 'preview', release: 'v0.3.0' },
  science: { stage: 'preview', release: 'main' },
  cloud: { stage: 'experimental', release: 'main' },
  site: { stage: 'released', release: 'live' },
  runtime: { stage: 'preview', release: 'v0.3.0' },
  'oci-runtime': { stage: 'experimental', release: 'v0.2.0' },
  flow: { stage: 'released', release: 'v1.0.0' },
  event: { stage: 'preview', release: 'v0.3.0' },
  lane: { stage: 'preview', release: 'v0.5.1' },
  memory: { stage: 'preview', release: 'v0.1.4' },
  orm: { stage: 'preview', release: 'v0.2.0' },
  common: { stage: 'preview', release: 'v0.1.1' },
  boot: { stage: 'preview', release: 'v0.1.3' },
  gateway: { stage: 'released', release: 'v1.0.13' },
  power: { stage: 'preview', release: 'v0.9.0' },
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
} as const satisfies Record<string, ProjectDeliveryStatus>;

const stageCopy: Record<Lang, Record<DeliveryStage, DeliveryStageCopy>> = {
  cn: {
    building: {
      label: '开发中',
      description: '主要使用路径还在建设，暂不作为公开入口。',
    },
    experimental: {
      label: '实验阶段',
      description: '需要明确启用或限定环境，关键发布门槛仍未完成。',
    },
    preview: {
      label: '预览阶段',
      description: '已有公开可用路径，但接口或兼容性仍可能调整。',
    },
    released: {
      label: '已发布',
      description: '已有维护中的公开版本或服务；部分功能仍可能处于预览或实验阶段。',
    },
  },
  en: {
    building: {
      label: 'Building',
      description: 'The main usage path is still under construction and is not yet a public entry point.',
    },
    experimental: {
      label: 'Experimental',
      description: 'Requires explicit opt-in or a qualified environment; key release gates remain open.',
    },
    preview: {
      label: 'Preview',
      description: 'A public usage path exists, but interfaces or compatibility may still change.',
    },
    released: {
      label: 'Released',
      description: 'A maintained public release or service exists; individual features may still be preview or experimental.',
    },
  },
};

export function getDeliveryStageCopy(stage: DeliveryStage, lang: Lang): DeliveryStageCopy {
  return stageCopy[lang][stage];
}

export function getProjectDeliveryStatus(
  projectId: string,
  lang: Lang,
): ProjectDeliveryStatus & DeliveryStageCopy {
  const project = statusByProject[projectId as keyof typeof statusByProject];
  if (!project) throw new Error(`Delivery status is missing for ecosystem project: ${projectId}`);

  return {
    stage: project.stage,
    release: project.release,
    ...getDeliveryStageCopy(project.stage, lang),
  };
}

export const statusProjectIds = Object.keys(statusByProject);
