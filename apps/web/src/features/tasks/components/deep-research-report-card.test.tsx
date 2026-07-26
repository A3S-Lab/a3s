import { render, screen } from '@testing-library/react';
import { expect, it } from 'vitest';
import { DeepResearchReportCard } from './deep-research-report-card';

it('opens run-scoped HTML and Markdown artifacts without accepting filesystem paths', () => {
  render(
    <DeepResearchReportCard
      sessionId='session/report'
      calls={[
        {
          id: 'deep-research-1',
          name: 'deep_research',
          state: 'succeeded',
          inputText: '',
          output: 'published',
          metadata: {
            report: {
              runId: 'web-run-123',
              status: 'synthesized',
              artifactKinds: ['markdown', 'html'],
            },
          },
        },
      ]}
    />
  );

  const report = screen.getByRole('link', { name: '打开网页版研究报告' });
  expect(report).toHaveAttribute(
    'href',
    '/api/v1/kernel/sessions/session%2Freport/research-artifact?runId=web-run-123&kind=html'
  );
  expect(report).toHaveAttribute('target', '_blank');
  expect(screen.getByText('综合报告已通过质量门槛')).toBeInTheDocument();

  expect(screen.getByRole('link', { name: '打开 Markdown 研究报告' })).toHaveAttribute(
    'href',
    '/api/v1/kernel/sessions/session%2Freport/research-artifact?runId=web-run-123&kind=markdown'
  );
});
