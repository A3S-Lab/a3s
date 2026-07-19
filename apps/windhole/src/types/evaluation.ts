export const EVALUATION_EFFORTS = ['none', 'minimal', 'low', 'medium', 'high', 'xhigh'] as const;

export type EvaluationEffort = (typeof EVALUATION_EFFORTS)[number];
