import JSZip from 'jszip';
import { describe, expect, it } from 'vitest';
import { createWorkArtifactBlob, importWorkFile } from './work-file-io';

const ONE_PIXEL_PNG = 'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=';

describe('Work XLSX worksheet image interoperability', () => {
  it('imports editable raster images with native worksheet anchors', async () => {
    const artifact = await importWorkFile(await createImageFixture());

    expect(artifact.content.type).toBe('spreadsheet');
    if (artifact.content.type !== 'spreadsheet') return;
    expect(artifact.content.sheets[0].images).toEqual([
      expect.objectContaining({
        id: expect.stringContaining('xlsx-image'),
        name: 'A3S mark',
        altText: 'Quarterly logo',
        contentType: 'image/png',
        src: expect.stringMatching(/^data:image\/png;base64,/),
        left: 96,
        top: 24,
        width: 288,
        height: 120,
      }),
    ]);
    expect(artifact.compatibility?.issues.find((issue) => issue.code === 'xlsx.images')).toMatchObject({
      severity: 'info',
      message: expect.stringContaining('preserved'),
    });
    expect(artifact.compatibility?.issues.some((issue) => issue.code === 'xlsx.drawings.unsupported')).toBe(false);
  });

  it('exports moved and resized images as native drawing parts and reopens them', async () => {
    const artifact = await importWorkFile(await createImageFixture());
    expect(artifact.content.type).toBe('spreadsheet');
    if (artifact.content.type !== 'spreadsheet') return;
    const image = artifact.content.sheets[0].images?.[0];
    expect(image).toBeDefined();
    if (!image) return;
    image.left = 192;
    image.top = 48;
    image.width = 192;
    image.height = 96;

    const exported = await createWorkArtifactBlob(artifact);
    const archive = await JSZip.loadAsync(exported);
    const worksheet = await archive.file('xl/worksheets/sheet1.xml')?.async('text');
    const drawing = await archive.file('xl/drawings/drawing1.xml')?.async('text');
    const drawingRelationships = await archive.file('xl/drawings/_rels/drawing1.xml.rels')?.async('text');

    expect(worksheet).toContain('<drawing r:id=');
    expect(drawing).toContain('name="A3S mark"');
    expect(drawing).toContain('descr="Quarterly logo"');
    expect(drawing).toContain('<xdr:from><xdr:col>2</xdr:col>');
    expect(drawing).toContain('<xdr:row>2</xdr:row>');
    expect(drawing).toContain('<xdr:to><xdr:col>4</xdr:col>');
    expect(drawing).toContain('<xdr:row>6</xdr:row>');
    expect(drawingRelationships).toContain('../media/image1.png');
    expect(archive.file('xl/media/image1.png')).not.toBeNull();

    const reopened = await importWorkFile(
      new File([exported], 'Image round trip.xlsx', {
        type: 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet',
      })
    );
    expect(reopened.content.type).toBe('spreadsheet');
    if (reopened.content.type !== 'spreadsheet') return;
    expect(reopened.content.sheets[0].images?.[0]).toMatchObject({
      name: 'A3S mark',
      altText: 'Quarterly logo',
      contentType: 'image/png',
      left: 192,
      top: 48,
      width: 192,
      height: 96,
    });
  });

  it('imports one-cell and absolute worksheet image anchors', async () => {
    const artifact = await importWorkFile(await createAlternateAnchorFixture());

    expect(artifact.content.type).toBe('spreadsheet');
    if (artifact.content.type !== 'spreadsheet') return;
    expect(artifact.content.sheets[0].images).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          name: 'One-cell image',
          left: 0,
          top: 0,
          width: 96,
          height: 48,
        }),
        expect.objectContaining({
          name: 'Absolute image',
          left: 96,
          top: 24,
          width: 192,
          height: 72,
        }),
      ])
    );
  });
});

async function createImageFixture(): Promise<File> {
  const XLSX = await import('xlsx');
  const workbook = XLSX.utils.book_new();
  XLSX.utils.book_append_sheet(
    workbook,
    XLSX.utils.aoa_to_sheet([
      ['Quarter', 'Revenue'],
      ['Q1', 42],
    ]),
    'Report'
  );
  const buffer = XLSX.write(workbook, { type: 'array', bookType: 'xlsx' }) as ArrayBuffer;
  const archive = await JSZip.loadAsync(buffer);
  const worksheetPath = 'xl/worksheets/sheet1.xml';
  const worksheet = await archive.file(worksheetPath)?.async('text');
  if (!worksheet) throw new Error('Fixture worksheet was not generated.');
  const withRelationshipNamespace = worksheet.includes('xmlns:r=')
    ? worksheet
    : worksheet.replace(
        '<worksheet ',
        '<worksheet xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" '
      );
  archive.file(
    worksheetPath,
    withRelationshipNamespace.replace('</worksheet>', '<drawing r:id="rIdImage"/></worksheet>')
  );
  archive.file(
    'xl/worksheets/_rels/sheet1.xml.rels',
    relationships([
      [
        'rIdImage',
        '../drawings/drawing1.xml',
        'http://schemas.openxmlformats.org/officeDocument/2006/relationships/drawing',
      ],
    ])
  );
  archive.file(
    'xl/drawings/drawing1.xml',
    [
      '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>',
      '<xdr:wsDr xmlns:xdr="http://schemas.openxmlformats.org/drawingml/2006/spreadsheetDrawing"',
      ' xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"',
      ' xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">',
      '<xdr:twoCellAnchor editAs="oneCell">',
      marker('from', 1, 1),
      marker('to', 4, 6),
      '<xdr:pic><xdr:nvPicPr><xdr:cNvPr id="2" name="A3S mark" descr="Quarterly logo"/>',
      '<xdr:cNvPicPr><a:picLocks noChangeAspect="1"/></xdr:cNvPicPr></xdr:nvPicPr>',
      '<xdr:blipFill><a:blip r:embed="rId1"/><a:stretch><a:fillRect/></a:stretch></xdr:blipFill>',
      '<xdr:spPr><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></xdr:spPr></xdr:pic>',
      '<xdr:clientData/></xdr:twoCellAnchor></xdr:wsDr>',
    ].join('')
  );
  archive.file(
    'xl/drawings/_rels/drawing1.xml.rels',
    relationships([
      ['rId1', '../media/image1.png', 'http://schemas.openxmlformats.org/officeDocument/2006/relationships/image'],
    ])
  );
  archive.file(
    'xl/media/image1.png',
    Uint8Array.from(atob(ONE_PIXEL_PNG), (character) => character.charCodeAt(0))
  );
  const contentTypes = await archive.file('[Content_Types].xml')?.async('text');
  if (!contentTypes) throw new Error('Fixture content types were not generated.');
  archive.file(
    '[Content_Types].xml',
    contentTypes.replace(
      '</Types>',
      '<Default Extension="png" ContentType="image/png"/>' +
        '<Override PartName="/xl/drawings/drawing1.xml" ' +
        'ContentType="application/vnd.openxmlformats-officedocument.drawing+xml"/></Types>'
    )
  );

  return new File([await archive.generateAsync({ type: 'arraybuffer' })], 'Images.xlsx', {
    type: 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet',
  });
}

async function createAlternateAnchorFixture(): Promise<File> {
  const archive = await JSZip.loadAsync(await (await createImageFixture()).arrayBuffer());
  const drawingPath = 'xl/drawings/drawing1.xml';
  const relationshipsPath = 'xl/drawings/_rels/drawing1.xml.rels';
  const drawing = await archive.file(drawingPath)?.async('text');
  const drawingRelationships = await archive.file(relationshipsPath)?.async('text');
  if (!drawing || !drawingRelationships) throw new Error('Fixture drawing was not generated.');
  archive.file(
    drawingPath,
    drawing.replace(
      '</xdr:wsDr>',
      [
        '<xdr:oneCellAnchor>',
        marker('from', 0, 0),
        '<xdr:ext cx="914400" cy="457200"/>',
        picture('rId2', 3, 'One-cell image'),
        '<xdr:clientData/></xdr:oneCellAnchor>',
        '<xdr:absoluteAnchor>',
        '<xdr:pos x="914400" y="228600"/><xdr:ext cx="1828800" cy="685800"/>',
        picture('rId3', 4, 'Absolute image'),
        '<xdr:clientData/></xdr:absoluteAnchor>',
        '</xdr:wsDr>',
      ].join('')
    )
  );
  archive.file(
    relationshipsPath,
    drawingRelationships.replace(
      '</Relationships>',
      [
        '<Relationship Id="rId2" Target="../media/image1.png"',
        ' Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image"/>',
        '<Relationship Id="rId3" Target="../media/image1.png"',
        ' Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image"/>',
        '</Relationships>',
      ].join('')
    )
  );
  return new File([await archive.generateAsync({ type: 'arraybuffer' })], 'Alternate anchors.xlsx', {
    type: 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet',
  });
}

function marker(kind: 'from' | 'to', column: number, row: number): string {
  return [
    `<xdr:${kind}>`,
    `<xdr:col>${column}</xdr:col><xdr:colOff>0</xdr:colOff>`,
    `<xdr:row>${row}</xdr:row><xdr:rowOff>0</xdr:rowOff>`,
    `</xdr:${kind}>`,
  ].join('');
}

function picture(relationshipId: string, id: number, name: string): string {
  return [
    '<xdr:pic><xdr:nvPicPr>',
    `<xdr:cNvPr id="${id}" name="${name}"/>`,
    '<xdr:cNvPicPr><a:picLocks noChangeAspect="1"/></xdr:cNvPicPr></xdr:nvPicPr>',
    `<xdr:blipFill><a:blip r:embed="${relationshipId}"/><a:stretch><a:fillRect/></a:stretch></xdr:blipFill>`,
    '<xdr:spPr><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></xdr:spPr></xdr:pic>',
  ].join('');
}

function relationships(items: Array<[string, string, string]>): string {
  return [
    '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>',
    '<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">',
    ...items.map(([id, target, type]) => `<Relationship Id="${id}" Target="${target}" Type="${type}"/>`),
    '</Relationships>',
  ].join('');
}
