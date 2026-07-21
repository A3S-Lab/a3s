import type { ReactNode } from 'react';
import { SlideCanvas } from '../editors/presentation-slide-canvas';
import type { WorkPresentationContent, WorkPresentationPrintLayout, WorkSlide } from '../work-types';

export function WorkPresentationPdfPages({
  content,
  layout,
}: {
  content: WorkPresentationContent;
  layout: WorkPresentationPrintLayout;
}) {
  const aspectRatio = `${content.width ?? 13.333} / ${content.height ?? 7.5}`;
  if (layout === 'slides') {
    return content.slides.map((slide, index) => (
      <PresentationPage
        key={slide.id}
        layout={layout}
        orientation='landscape'
        pageIndex={index}
        slideCount={content.slides.length}
      >
        <SlideCanvas content={content} slide={slide} interactive={false} aspectRatio={aspectRatio} />
      </PresentationPage>
    ));
  }
  if (layout === 'notes') {
    return content.slides.map((slide, index) => (
      <PresentationPage
        key={slide.id}
        layout={layout}
        orientation='portrait'
        pageIndex={index}
        slideCount={content.slides.length}
      >
        <header className='work-presentation-print-heading'>
          <strong>{slide.name}</strong>
          <span>
            {index + 1} / {content.slides.length}
          </span>
        </header>
        <div className='work-presentation-print-notes-slide'>
          <SlideCanvas content={content} slide={slide} interactive={false} aspectRatio={aspectRatio} />
        </div>
        <section className='work-presentation-print-notes' aria-label={`第 ${index + 1} 张演讲者备注`}>
          <h2>演讲者备注</h2>
          <p>{slide.notes?.trim() || '此页没有演讲者备注'}</p>
        </section>
      </PresentationPage>
    ));
  }

  const pageSize = handoutPageSize(layout);
  const orientation = layout === 'handout-6' ? 'landscape' : 'portrait';
  return chunks(content.slides, pageSize).map((slides, pageIndex) => (
    <PresentationPage
      key={`${layout}-${pageIndex}`}
      layout={layout}
      orientation={orientation}
      pageIndex={pageIndex}
      slideCount={content.slides.length}
    >
      <header className='work-presentation-print-heading'>
        <strong>演示讲义</strong>
        <span>
          第 {pageIndex + 1} 页 · 共 {Math.ceil(content.slides.length / pageSize)} 页
        </span>
      </header>
      <div className={`work-presentation-handout-grid ${layout}`}>
        {slides.map((slide, index) => (
          <HandoutSlide
            key={slide.id}
            slide={slide}
            content={content}
            number={pageIndex * pageSize + index + 1}
            aspectRatio={aspectRatio}
            ruled={layout === 'handout-3'}
          />
        ))}
      </div>
    </PresentationPage>
  ));
}

function PresentationPage({
  layout,
  orientation,
  pageIndex,
  slideCount,
  children,
}: {
  layout: WorkPresentationPrintLayout;
  orientation: 'portrait' | 'landscape';
  pageIndex: number;
  slideCount: number;
  children: ReactNode;
}) {
  return (
    <section
      className={`work-pdf-export-page presentation-print ${layout} ${orientation}`}
      data-work-pdf-page
      data-pdf-orientation={orientation}
      data-pdf-page-size='a4'
      data-presentation-print-layout={layout}
      data-presentation-print-page={pageIndex + 1}
      data-presentation-slide-count={slideCount}
      aria-label={`演示打印预览第 ${pageIndex + 1} 页`}
    >
      {children}
    </section>
  );
}

function HandoutSlide({
  content,
  slide,
  number,
  aspectRatio,
  ruled,
}: {
  content: WorkPresentationContent;
  slide: WorkSlide;
  number: number;
  aspectRatio: string;
  ruled: boolean;
}) {
  return (
    <article className='work-presentation-handout-slide' data-presentation-slide-number={number}>
      <span>{number}</span>
      <div>
        <SlideCanvas content={content} slide={slide} interactive={false} aspectRatio={aspectRatio} />
        <strong>{slide.name}</strong>
      </div>
      {ruled && <div className='work-presentation-handout-lines' aria-hidden='true' />}
    </article>
  );
}

function handoutPageSize(layout: Exclude<WorkPresentationPrintLayout, 'slides' | 'notes'>): number {
  if (layout === 'handout-2') return 2;
  if (layout === 'handout-3') return 3;
  return 6;
}

function chunks<T>(items: T[], size: number): T[][] {
  return Array.from({ length: Math.ceil(items.length / size) }, (_, index) =>
    items.slice(index * size, (index + 1) * size)
  );
}
