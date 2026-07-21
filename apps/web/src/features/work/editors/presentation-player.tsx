import { ChevronLeft, ChevronRight, Maximize2, Presentation } from 'lucide-react';
import { useCallback, useEffect, useRef, useState } from 'react';
import { slideTransitionDurationMilliseconds } from '../work-presentation-transition';
import type { WorkPresentationContent } from '../work-types';
import { PresentationPresenterView } from './presentation-presenter-view';
import { SlideCanvas } from './presentation-slide-canvas';

interface PlaybackState {
  index: number;
  transitionKey: number;
}

export function PresentationPlayer({ content }: { content: WorkPresentationContent }) {
  const [playback, setPlayback] = useState<PlaybackState>({ index: 0, transitionKey: 0 });
  const [presenter, setPresenter] = useState(false);
  const playerRef = useRef<HTMLDivElement>(null);
  const slide = content.slides[playback.index] ?? content.slides[0];
  const move = useCallback(
    (delta: number) => {
      setPlayback((current) => {
        const index = Math.min(Math.max(current.index + delta, 0), content.slides.length - 1);
        return index === current.index ? current : { index, transitionKey: current.transitionKey + 1 };
      });
    },
    [content.slides.length]
  );
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (
        event.target instanceof HTMLButtonElement ||
        event.target instanceof HTMLAnchorElement ||
        event.target instanceof HTMLInputElement
      ) {
        return;
      }
      if (event.key === 'ArrowRight' || event.key === ' ') move(1);
      if (event.key === 'ArrowLeft') move(-1);
      if (event.key === 'Escape' && document.fullscreenElement) void document.exitFullscreen();
    };
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, [move]);
  useEffect(() => {
    const delay = slide?.transition?.advanceAfterMs;
    if (delay === undefined || playback.index >= content.slides.length - 1) return;
    const timer = window.setTimeout(() => move(1), delay);
    return () => window.clearTimeout(timer);
  }, [content.slides.length, move, playback.index, slide?.transition?.advanceAfterMs]);
  if (!slide) return null;
  const aspectRatio = `${content.width ?? 13.333} / ${content.height ?? 7.5}`;
  const transition = slide.transition;
  const transitionStyle = {
    '--work-slide-transition-duration': `${slideTransitionDurationMilliseconds(transition)}ms`,
  } as React.CSSProperties;
  return (
    <section
      className='work-presentation-player'
      data-player-mode={presenter ? 'presenter' : 'audience'}
      ref={playerRef}
    >
      {presenter ? (
        <PresentationPresenterView
          content={content}
          slide={slide}
          nextSlide={content.slides[playback.index + 1]}
          index={playback.index}
          total={content.slides.length}
          aspectRatio={aspectRatio}
          onMove={move}
        />
      ) : (
        <div className='work-presentation-player-stage'>
          <button
            type='button'
            className='work-presentation-player-advance'
            aria-label='单击换到下一张幻灯片'
            disabled={transition?.advanceOnClick === false || playback.index === content.slides.length - 1}
            onClick={() => move(1)}
          />
          <div
            aria-live='polite'
            className='work-presentation-transition-layer'
            data-slide-index={playback.index}
            data-slide-transition={transition?.type ?? 'none'}
            data-transition-direction={transition?.direction}
            data-transition-orientation={transition?.orientation}
            data-transition-speed={transition?.speed ?? 'medium'}
            key={`${slide.id}-${playback.transitionKey}`}
            style={transitionStyle}
          >
            <SlideCanvas content={content} slide={slide} interactive={false} aspectRatio={aspectRatio} />
          </div>
        </div>
      )}
      <footer>
        <button type='button' aria-label='上一张' disabled={playback.index === 0} onClick={() => move(-1)}>
          <ChevronLeft size={18} />
        </button>
        <span>
          {playback.index + 1} / {content.slides.length}
        </span>
        <button
          type='button'
          aria-label='下一张'
          disabled={playback.index === content.slides.length - 1}
          onClick={() => move(1)}
        >
          <ChevronRight size={18} />
        </button>
        <button
          type='button'
          className={presenter ? 'active' : ''}
          aria-label={presenter ? '退出演讲者视图' : '演讲者视图'}
          onClick={() => setPresenter((current) => !current)}
        >
          <Presentation size={16} />
        </button>
        <button
          type='button'
          className='work-presentation-player-fullscreen'
          aria-label='全屏放映'
          onClick={() => void playerRef.current?.requestFullscreen()}
        >
          <Maximize2 size={16} />
        </button>
      </footer>
    </section>
  );
}
