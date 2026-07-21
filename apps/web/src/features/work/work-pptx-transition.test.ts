import { describe, expect, it } from 'vitest';
import { parseXml } from './work-ooxml-package';
import { readPptxTransition } from './work-pptx-transition';

const NAMESPACE = 'http://schemas.openxmlformats.org/presentationml/2006/main';

describe('Work PPTX transitions', () => {
  it.each([
    [
      '<p:fade/>',
      {
        type: 'fade',
        speed: 'fast',
        advanceOnClick: false,
        advanceAfterMs: 2500,
      },
    ],
    ['<p:push dir="r"/>', { type: 'push', speed: 'medium', direction: 'right', advanceOnClick: true }],
    ['<p:wipe dir="u"/>', { type: 'wipe', speed: 'medium', direction: 'up', advanceOnClick: true }],
    [
      '<p:split orient="vert" dir="in"/>',
      {
        type: 'split',
        speed: 'medium',
        direction: 'in',
        orientation: 'vertical',
        advanceOnClick: true,
      },
    ],
    ['<p:cut/>', { type: 'cut', speed: 'medium', advanceOnClick: true }],
  ])('parses supported transition %s', (effect, expected) => {
    const attributes = effect === '<p:fade/>' ? ' spd="fast" advClick="0" advTm="2500"' : '';
    const document = parseXml(
      `<p:sld xmlns:p="${NAMESPACE}"><p:cSld/><p:transition${attributes}>${effect}</p:transition></p:sld>`
    );

    expect(readPptxTransition(document)).toEqual({
      present: true,
      transition: expected,
      diagnostics: [],
    });
  });

  it('reports unsupported effects instead of silently treating them as editable', () => {
    const document = parseXml(
      `<p:sld xmlns:p="${NAMESPACE}"><p:cSld/><p:transition><p:randomBar/></p:transition></p:sld>`
    );

    expect(readPptxTransition(document)).toEqual({
      present: true,
      diagnostics: [
        {
          code: 'pptx.transition.type',
          message: 'The “randomBar” slide transition remains in the original PPTX only.',
        },
      ],
    });
  });
});
