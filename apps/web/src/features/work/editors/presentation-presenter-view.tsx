import { Pause, Play, RotateCcw } from 'lucide-react';
import { useEffect, useState } from 'react';
import type { WorkPresentationContent, WorkSlide } from '../work-types';
import { SlideCanvas } from './presentation-slide-canvas';

export function PresentationPresenterView({
  content,
  slide,
  nextSlide,
  index,
  total,
  aspectRatio,
  onMove,
}: {
  content: WorkPresentationContent;
  slide: WorkSlide;
  nextSlide?: WorkSlide;
  index: number;
  total: number;
  aspectRatio: string;
  onMove: (delta: number) => void;
}) {
  const [elapsedSeconds, setElapsedSeconds] = useState(0);
  const [running, setRunning] = useState(true);

  useEffect(() => {
    if (!running) return;
    const timer = window.setInterval(() => setElapsedSeconds((current) => current + 1), 1000);
    return () => window.clearInterval(timer);
  }, [running]);

  return (
    <section className='work-presentation-presenter' aria-label='演讲者视图'>
      <header>
        <div>
          <span>演讲计时</span>
          <strong>
            <span className='sr-only'>已用时间：</span>
            <time>{formatDuration(elapsedSeconds)}</time>
          </strong>
        </div>
        <div className='work-presentation-presenter-timer-actions'>
          <button
            type='button'
            aria-label={running ? '暂停计时' : '继续计时'}
            onClick={() => setRunning((current) => !current)}
          >
            {running ? <Pause size={15} /> : <Play size={15} />}
          </button>
          <button type='button' aria-label='重置计时' onClick={() => setElapsedSeconds(0)}>
            <RotateCcw size={15} />
          </button>
        </div>
        <output aria-live='polite'>
          幻灯片 {index + 1} / {total}
        </output>
      </header>

      <div className='work-presentation-presenter-grid'>
        <section className='work-presentation-presenter-current' aria-label='当前幻灯片'>
          <h2>{slide.name}</h2>
          <SlideCanvas content={content} slide={slide} interactive={false} aspectRatio={aspectRatio} />
        </section>
        <section className='work-presentation-presenter-next' aria-label='下一张幻灯片'>
          <h2>下一张</h2>
          {nextSlide ? (
            <>
              <SlideCanvas content={content} slide={nextSlide} interactive={false} aspectRatio={aspectRatio} />
              <span>{nextSlide.name}</span>
            </>
          ) : (
            <p>演示结束</p>
          )}
        </section>
        <aside className='work-presentation-presenter-notes' aria-label='演讲者备注'>
          <h2>演讲者备注</h2>
          <p>{slide.notes?.trim() || '此页没有演讲者备注'}</p>
        </aside>
      </div>

      <footer>
        <button type='button' aria-label='演讲者上一张' disabled={index === 0} onClick={() => onMove(-1)}>
          上一张
        </button>
        <button type='button' aria-label='演讲者下一张' disabled={index === total - 1} onClick={() => onMove(1)}>
          下一张
        </button>
      </footer>
    </section>
  );
}

function formatDuration(seconds: number): string {
  const minutes = Math.floor(seconds / 60);
  const remainder = seconds % 60;
  return `${String(minutes).padStart(2, '0')}:${String(remainder).padStart(2, '0')}`;
}
