import { describe, expect, it } from 'vitest';
import { applyPresentationLayout, presentationSlideView, withPresentationDesign } from './work-presentation-layouts';
import type {
  WorkPresentationContent,
  WorkPresentationLayout,
  WorkPresentationMaster,
  WorkSlideElement,
} from './work-types';

const master: WorkPresentationMaster = {
  id: 'master-a3s',
  name: 'A3S 母版',
  background: '#16213d',
  elements: [
    element('master-accent', { x: 0, y: 94, width: 100, height: 6, fill: '#ffb15a' }),
    element('master-footer', {
      x: 5,
      y: 90,
      width: 30,
      height: 4,
      text: '页脚',
      placeholder: { key: 'type:ftr', type: 'ftr', prompt: '页脚' },
    }),
  ],
};

const titleLayout: WorkPresentationLayout = {
  id: 'layout-title',
  name: '标题',
  masterId: master.id,
  elements: [
    element('layout-rule', { x: 8, y: 24, width: 84, height: 1, fill: '#ffb15a' }),
    element('layout-title-placeholder', {
      x: 8,
      y: 10,
      width: 84,
      height: 12,
      text: '单击添加标题',
      placeholder: { key: 'idx:1', type: 'title', prompt: '单击添加标题' },
    }),
  ],
};

const contentLayout: WorkPresentationLayout = {
  id: 'layout-content',
  name: '标题和内容',
  masterId: master.id,
  background: '#f7f4ee',
  elements: [
    element('layout-content-title', {
      x: 8,
      y: 8,
      width: 84,
      height: 10,
      text: '单击添加标题',
      placeholder: { key: 'idx:1', type: 'title', prompt: '单击添加标题' },
    }),
    element('layout-body-placeholder', {
      x: 10,
      y: 24,
      width: 80,
      height: 58,
      text: '单击添加内容',
      placeholder: { key: 'idx:2', type: 'body', prompt: '单击添加内容' },
    }),
  ],
};

describe('Work presentation layouts and masters', () => {
  it('composes master and layout artwork behind slide content', () => {
    const content = presentationContent();
    const slide = content.slides[0];
    const view = presentationSlideView(content, slide);

    expect(view.background).toBe('#f7f4ee');
    expect(view.master?.id).toBe(master.id);
    expect(view.layout?.id).toBe(contentLayout.id);
    expect(view.inheritedElements.map((item) => item.id)).toEqual(['master-accent']);
    expect(view.placeholderElements.map((item) => item.placeholder?.key)).toEqual(['type:ftr', 'idx:1', 'idx:2']);
  });

  it('applies a layout without dropping unmatched placeholder content', () => {
    const content = presentationContent();
    const changed = applyPresentationLayout(content, 'slide-1', titleLayout.id);
    const slide = changed.slides[0];
    const title = slide.elements.find((item) => item.id === 'slide-title');
    const formerBody = slide.elements.find((item) => item.id === 'slide-body');

    expect(slide.layoutId).toBe(titleLayout.id);
    expect(title).toMatchObject({
      text: 'Quarterly Review',
      x: 8,
      y: 10,
      width: 84,
      height: 12,
      placeholder: { key: 'idx:1', type: 'title' },
    });
    expect(formerBody).toMatchObject({
      text: 'Keep this body copy',
      placeholder: undefined,
    });
  });

  it('adds a backward-compatible default design without overriding slide backgrounds', () => {
    const legacy: WorkPresentationContent = {
      type: 'presentation',
      slides: [
        {
          id: 'legacy-slide',
          name: 'Legacy',
          background: '#334455',
          elements: [],
        },
      ],
    };
    const normalized = withPresentationDesign(legacy);

    expect(normalized.masters).toHaveLength(1);
    expect(normalized.layouts).toHaveLength(1);
    expect(normalized.slides[0]).toMatchObject({
      layoutId: normalized.layouts?.[0].id,
      background: '#334455',
      useLayoutBackground: false,
    });
    expect(presentationSlideView(normalized, normalized.slides[0]).background).toBe('#334455');
  });
});

function presentationContent(): WorkPresentationContent {
  return {
    type: 'presentation',
    masters: [master],
    layouts: [titleLayout, contentLayout],
    slides: [
      {
        id: 'slide-1',
        name: 'Review',
        background: '#ffffff',
        layoutId: contentLayout.id,
        useLayoutBackground: true,
        elements: [
          element('slide-title', {
            text: 'Quarterly Review',
            placeholder: { key: 'idx:1', type: 'title' },
          }),
          element('slide-body', {
            text: 'Keep this body copy',
            placeholder: { key: 'idx:2', type: 'body' },
          }),
        ],
      },
    ],
  };
}

function element(id: string, patch: Partial<WorkSlideElement> = {}): WorkSlideElement {
  return {
    id,
    type: 'text',
    x: 12,
    y: 12,
    width: 40,
    height: 10,
    text: '',
    fontSize: 24,
    color: '#ffffff',
    fill: 'transparent',
    bold: false,
    align: 'left',
    ...patch,
  };
}
