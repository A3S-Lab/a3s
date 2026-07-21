import JSZip from 'jszip';
import PptxGenJS from 'pptxgenjs';
import { describe, expect, it } from 'vitest';
import { importWorkFile } from './work-file-io';
import { createPptxBlob } from './work-pptx-export';
import { presentationSlideView } from './work-presentation-layouts';

describe('Work PPTX layout and master interoperability', () => {
  it('imports masters, layouts, inherited artwork, and placeholder bindings', async () => {
    const artifact = await importWorkFile(await createLayoutFixture());

    expect(artifact.content.type).toBe('presentation');
    if (artifact.content.type !== 'presentation') return;
    expect(artifact.content.masters).toHaveLength(1);
    expect(artifact.content.layouts?.map((layout) => layout.name)).toEqual([
      'Title layout',
      'Content layout',
      'Blank layout',
    ]);
    expect(artifact.content.masters?.[0]).toMatchObject({
      name: 'A3S master',
      background: '#16213d',
    });
    expect(artifact.content.masters?.[0].elements).toEqual([
      expect.objectContaining({
        type: 'shape',
        fill: '#ffb15a',
        x: 0,
        y: 94,
        width: 100,
        height: 6,
      }),
    ]);

    const [titleSlide, contentSlide] = artifact.content.slides;
    expect(titleSlide.layoutId).toBe(artifact.content.layouts?.[0].id);
    expect(contentSlide.layoutId).toBe(artifact.content.layouts?.[1].id);
    expect(titleSlide.useLayoutBackground).toBe(true);
    expect(contentSlide.useLayoutBackground).toBe(true);
    expect(titleSlide.elements[0]).toMatchObject({
      text: 'Quarterly Review',
      x: 8,
      y: 10,
      width: 84,
      fontSize: 28,
      placeholder: { key: 'idx:1', type: 'title' },
    });
    expect(titleSlide.elements[0].textRuns?.[0].fontSize).toBe(28);
    expect(titleSlide.elements[0].height).toBeCloseTo(14);
    expect(contentSlide.elements[0]).toMatchObject({
      text: 'Evidence',
      x: 10,
      y: 24,
      width: 80,
      placeholder: { key: 'idx:2', type: 'body' },
    });
    expect(contentSlide.elements[0].height).toBeCloseTo(58);
    expect(presentationSlideView(artifact.content, titleSlide).background).toBe('#16213d');
    expect(presentationSlideView(artifact.content, contentSlide).background).toBe('#f7f4ee');
    expect(artifact.compatibility?.issues.find((issue) => issue.code === 'pptx.layouts')).toMatchObject({
      severity: 'info',
      message: expect.stringContaining('3 slide layout'),
    });
  });

  it('exports editable layouts as native layout parts and reopens their visual inheritance', async () => {
    const artifact = await importWorkFile(await createLayoutFixture());
    const exported = await createPptxBlob(artifact, PptxGenJS);
    const archive = await JSZip.loadAsync(exported);
    const layoutPaths = Object.keys(archive.files).filter((path) =>
      /^ppt\/slideLayouts\/slideLayout\d+\.xml$/.test(path)
    );
    const layoutXml = await Promise.all(layoutPaths.map(async (path) => archive.file(path)?.async('text')));

    expect(layoutXml.some((source) => source?.includes('name="Title layout"'))).toBe(true);
    expect(layoutXml.some((source) => source?.includes('name="Content layout"'))).toBe(true);
    expect(layoutXml.some((source) => source?.includes('val="F7F4EE"'))).toBe(true);
    expect(layoutXml.filter((source) => source?.includes('<p:ph')).length).toBeGreaterThanOrEqual(2);

    const reopened = await importWorkFile(
      new File([exported], 'Layout round trip.pptx', {
        type: 'application/vnd.openxmlformats-officedocument.presentationml.presentation',
      })
    );
    expect(reopened.content.type).toBe('presentation');
    if (reopened.content.type !== 'presentation') return;
    const content = reopened.content;

    expect(content.layouts?.map((layout) => layout.name)).toEqual(
      expect.arrayContaining(['Title layout', 'Content layout'])
    );
    expect(content.slides.map((slide) => presentationSlideView(content, slide).background)).toEqual([
      '#16213d',
      '#f7f4ee',
    ]);
    expect(
      content.slides.map((slide) =>
        presentationSlideView(content, slide).inheritedElements.some((element) => element.fill === '#ffb15a')
      )
    ).toEqual([true, true]);
    expect(content.slides[0].elements.find((element) => element.text === 'Quarterly Review')?.placeholder).toEqual(
      definedPlaceholder()
    );
  });
});

async function createLayoutFixture(): Promise<File> {
  const archive = new JSZip();
  archive.file(
    'ppt/presentation.xml',
    xml`
      <p:presentation xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"
        xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
        <p:sldIdLst>
          <p:sldId id="256" r:id="rId1"/>
          <p:sldId id="257" r:id="rId2"/>
        </p:sldIdLst>
        <p:sldSz cx="12192000" cy="6858000"/>
      </p:presentation>
    `
  );
  archive.file(
    'ppt/_rels/presentation.xml.rels',
    relationships([
      ['rId1', 'slides/slide1.xml', 'slide'],
      ['rId2', 'slides/slide2.xml', 'slide'],
      ['rId3', 'slideMasters/slideMaster1.xml', 'slideMaster'],
    ])
  );
  archive.file(
    'ppt/slideMasters/slideMaster1.xml',
    xml`
      <p:sldMaster xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"
        xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
        xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
        <p:cSld name="A3S master">
          <p:bg><p:bgPr><a:solidFill><a:srgbClr val="16213D"/></a:solidFill></p:bgPr></p:bg>
          <p:spTree>
            ${shapeXml('Master accent', 0, 6446520, 12192000, 411480, 'FFB15A')}
          </p:spTree>
        </p:cSld>
        <p:sldLayoutIdLst>
          <p:sldLayoutId id="2147483649" r:id="rId1"/>
          <p:sldLayoutId id="2147483650" r:id="rId2"/>
          <p:sldLayoutId id="2147483651" r:id="rId3"/>
        </p:sldLayoutIdLst>
      </p:sldMaster>
    `
  );
  archive.file(
    'ppt/slideMasters/_rels/slideMaster1.xml.rels',
    relationships([
      ['rId1', '../slideLayouts/slideLayout1.xml', 'slideLayout'],
      ['rId2', '../slideLayouts/slideLayout2.xml', 'slideLayout'],
      ['rId3', '../slideLayouts/slideLayout3.xml', 'slideLayout'],
    ])
  );
  archive.file(
    'ppt/slideLayouts/slideLayout1.xml',
    layoutXml('Title layout', 'title', '1', 975360, 685800, 10241280, 960120)
  );
  archive.file(
    'ppt/slideLayouts/slideLayout2.xml',
    layoutXml('Content layout', 'body', '2', 1219200, 1645920, 9753600, 3977640, 'F7F4EE')
  );
  archive.file(
    'ppt/slideLayouts/_rels/slideLayout1.xml.rels',
    relationships([['rId1', '../slideMasters/slideMaster1.xml', 'slideMaster']])
  );
  archive.file(
    'ppt/slideLayouts/_rels/slideLayout2.xml.rels',
    relationships([['rId1', '../slideMasters/slideMaster1.xml', 'slideMaster']])
  );
  archive.file(
    'ppt/slideLayouts/slideLayout3.xml',
    xml`
      <p:sldLayout xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"
        xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
        <p:cSld name="Blank layout"><p:spTree/></p:cSld>
      </p:sldLayout>
    `
  );
  archive.file(
    'ppt/slideLayouts/_rels/slideLayout3.xml.rels',
    relationships([['rId1', '../slideMasters/slideMaster1.xml', 'slideMaster']])
  );
  archive.file('ppt/slides/slide1.xml', slideXml('Title slide', 'title', '1', 'Quarterly Review'));
  archive.file('ppt/slides/slide2.xml', slideXml('Content slide', 'body', '2', 'Evidence'));
  archive.file(
    'ppt/slides/_rels/slide1.xml.rels',
    relationships([['rId1', '../slideLayouts/slideLayout1.xml', 'slideLayout']])
  );
  archive.file(
    'ppt/slides/_rels/slide2.xml.rels',
    relationships([['rId1', '../slideLayouts/slideLayout2.xml', 'slideLayout']])
  );
  return new File([await archive.generateAsync({ type: 'arraybuffer' })], 'Layouts.pptx', {
    type: 'application/vnd.openxmlformats-officedocument.presentationml.presentation',
  });
}

function layoutXml(
  name: string,
  type: string,
  index: string,
  x: number,
  y: number,
  width: number,
  height: number,
  background?: string
): string {
  return xml`
    <p:sldLayout xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"
      xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
      <p:cSld name="${name}">
        ${background ? `<p:bg><p:bgPr><a:solidFill><a:srgbClr val="${background}"/></a:solidFill></p:bgPr></p:bg>` : ''}
        <p:spTree>
          <p:sp>
            <p:nvSpPr><p:cNvPr id="2" name="${name} placeholder"/><p:cNvSpPr/>
              <p:nvPr><p:ph type="${type}" idx="${index}"/></p:nvPr>
            </p:nvSpPr>
            <p:spPr><a:xfrm><a:off x="${x}" y="${y}"/><a:ext cx="${width}" cy="${height}"/></a:xfrm>
              <a:prstGeom prst="rect"/><a:noFill/>
            </p:spPr>
            <p:txBody><a:bodyPr/><a:p><a:r><a:rPr sz="2800"/><a:t>Click to add ${type}</a:t></a:r></a:p></p:txBody>
          </p:sp>
        </p:spTree>
      </p:cSld>
    </p:sldLayout>
  `;
}

function slideXml(name: string, type: string, index: string, text: string): string {
  return xml`
    <p:sld xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"
      xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
      <p:cSld name="${name}"><p:spTree><p:sp>
        <p:nvSpPr><p:cNvPr id="2" name="${name} placeholder"/><p:cNvSpPr/>
          <p:nvPr><p:ph type="${type}" idx="${index}"/></p:nvPr>
        </p:nvSpPr>
        <p:spPr><a:noFill/></p:spPr>
        <p:txBody><a:bodyPr/><a:p><a:r><a:t>${text}</a:t></a:r></a:p></p:txBody>
      </p:sp></p:spTree></p:cSld>
    </p:sld>
  `;
}

function shapeXml(name: string, x: number, y: number, width: number, height: number, fill: string): string {
  return [
    '<p:sp><p:nvSpPr>',
    `<p:cNvPr id="3" name="${name}"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr>`,
    `<p:spPr><a:xfrm><a:off x="${x}" y="${y}"/><a:ext cx="${width}" cy="${height}"/></a:xfrm>`,
    `<a:prstGeom prst="rect"/><a:solidFill><a:srgbClr val="${fill}"/></a:solidFill></p:spPr>`,
    '</p:sp>',
  ].join('');
}

function relationships(items: Array<[id: string, target: string, kind: string]>): string {
  return xml`
    <Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
      ${items
        .map(
          ([id, target, kind]) =>
            `<Relationship Id="${id}" Target="${target}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/${kind}"/>`
        )
        .join('')}
    </Relationships>
  `;
}

function xml(strings: TemplateStringsArray, ...values: unknown[]): string {
  return String.raw({ raw: strings }, ...values)
    .replace(/>\s+</g, '><')
    .trim();
}

function definedPlaceholder(): unknown {
  return expect.objectContaining({ key: expect.any(String), type: 'title' });
}
