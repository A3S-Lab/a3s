import {
  useCallback,
  useEffect,
  useRef,
  useState,
  useSyncExternalStore,
} from 'react';
import {
  cliTerminalInterfaceCopy as interfaceCopy,
  cliTerminalScenarios as scenarios,
  type CliTerminalLocale,
} from './cli-terminal-content';
import { A3sCodeTui } from './A3sCodeTui';

type TerminalPhase = 'typing' | 'output' | 'complete';

const reducedMotionQuery = '(prefers-reduced-motion: reduce)';

function subscribeToReducedMotion(onStoreChange: () => void) {
  const query = window.matchMedia(reducedMotionQuery);
  query.addEventListener('change', onStoreChange);
  return () => query.removeEventListener('change', onStoreChange);
}

function getReducedMotionSnapshot() {
  return window.matchMedia(reducedMotionQuery).matches;
}

function getReducedMotionServerSnapshot() {
  return true;
}

function ReplayIcon() {
  return (
    <svg aria-hidden="true" viewBox="0 0 20 20">
      <path d="M15.5 7.1A6 6 0 1 0 16 12" />
      <path d="M15.5 3.8v3.7h-3.7" />
    </svg>
  );
}

export function CliTerminalShowcase({ locale }: { locale: CliTerminalLocale }) {
  const rootRef = useRef<HTMLElement>(null);
  const reducedMotion = useSyncExternalStore(
    subscribeToReducedMotion,
    getReducedMotionSnapshot,
    getReducedMotionServerSnapshot,
  );
  const [activeIndex, setActiveIndex] = useState(0);
  const [phase, setPhase] = useState<TerminalPhase>('complete');
  const [typedCharacters, setTypedCharacters] = useState(
    scenarios[0].command.length,
  );
  const [visibleLines, setVisibleLines] = useState(scenarios[0].output.length);
  const [playing, setPlaying] = useState(true);
  const [inView, setInView] = useState(true);
  const [pageVisible, setPageVisible] = useState(true);

  const activeScenario = scenarios[activeIndex];
  const ui = interfaceCopy[locale];
  const canAnimate = playing && inView && pageVisible && !reducedMotion;

  const restart = useCallback(() => {
    if (reducedMotion) {
      setTypedCharacters(activeScenario.command.length);
      setVisibleLines(activeScenario.output.length);
      setPhase('complete');
      return;
    }

    setTypedCharacters(0);
    setVisibleLines(0);
    setPhase('typing');
    setPlaying(true);
  }, [
    activeScenario.command.length,
    activeScenario.output.length,
    reducedMotion,
  ]);

  useEffect(() => {
    restart();
  }, [locale, restart]);

  useEffect(() => {
    const element = rootRef.current;
    if (!element || typeof IntersectionObserver === 'undefined') return;

    const observer = new IntersectionObserver(
      ([entry]) => setInView(entry?.isIntersecting ?? true),
      { rootMargin: '80px' },
    );
    observer.observe(element);
    return () => observer.disconnect();
  }, []);

  useEffect(() => {
    function updatePageVisibility() {
      setPageVisible(document.visibilityState !== 'hidden');
    }

    updatePageVisibility();
    document.addEventListener('visibilitychange', updatePageVisibility);
    return () =>
      document.removeEventListener('visibilitychange', updatePageVisibility);
  }, []);

  useEffect(() => {
    if (!canAnimate || phase !== 'typing') return;

    if (typedCharacters >= activeScenario.command.length) {
      const timeout = window.setTimeout(() => setPhase('output'), 220);
      return () => window.clearTimeout(timeout);
    }

    const delay = typedCharacters === 0 ? 280 : 24;
    const timeout = window.setTimeout(
      () => setTypedCharacters((current) => current + 1),
      delay,
    );
    return () => window.clearTimeout(timeout);
  }, [activeScenario.command.length, canAnimate, phase, typedCharacters]);

  useEffect(() => {
    if (!canAnimate || phase !== 'output') return;

    if (visibleLines >= activeScenario.output.length) {
      const timeout = window.setTimeout(() => setPhase('complete'), 260);
      return () => window.clearTimeout(timeout);
    }

    const delay = visibleLines === 0 ? 360 : 310;
    const timeout = window.setTimeout(
      () => setVisibleLines((current) => current + 1),
      delay,
    );
    return () => window.clearTimeout(timeout);
  }, [activeScenario.output.length, canAnimate, phase, visibleLines]);

  useEffect(() => {
    if (!canAnimate || phase !== 'complete') return;

    const timeout = window.setTimeout(() => {
      setActiveIndex((current) => (current + 1) % scenarios.length);
      setTypedCharacters(0);
      setVisibleLines(0);
      setPhase('typing');
    }, 3_600);
    return () => window.clearTimeout(timeout);
  }, [canAnimate, phase]);

  function selectScenario(index: number) {
    const scenario = scenarios[index];
    setActiveIndex(index);
    setPlaying(true);

    if (reducedMotion) {
      setTypedCharacters(scenario.command.length);
      setVisibleLines(scenario.output.length);
      setPhase('complete');
    } else {
      setTypedCharacters(0);
      setVisibleLines(0);
      setPhase('typing');
    }
  }

  const status = reducedMotion
    ? ui.ready
    : !playing
      ? ui.paused
      : phase === 'complete'
        ? ui.ready
        : ui.running;

  return (
    <A3sCodeTui
      ref={rootRef}
      ariaLabel={ui.region}
      assistiveText={`${activeScenario.command}. ${activeScenario.summary[locale]}.`}
      className="cli-terminal--hero"
      controls={
        <button
          aria-label={playing ? ui.pause : ui.play}
          disabled={reducedMotion}
          onClick={() => setPlaying((current) => !current)}
          title={reducedMotion ? ui.reduced : playing ? ui.pause : ui.play}
          type="button"
        >
          <span aria-hidden="true">{playing ? 'Ⅱ' : '▶'}</span>
        </button>
      }
      footerAction={
        <button
          aria-label={ui.replayLabel}
          disabled={reducedMotion}
          onClick={restart}
          title={reducedMotion ? ui.reduced : ui.replayLabel}
          type="button"
        >
          <ReplayIcon />
          {ui.replay}
        </button>
      }
      footerLead={`${String(activeIndex + 1).padStart(2, '0')} / ${String(scenarios.length).padStart(2, '0')}`}
      footerMeta={activeScenario.label}
      prompt={activeScenario.command.slice(0, typedCharacters)}
      promptActive={phase === 'typing' && !reducedMotion}
      promptPrefix="$"
      showcase
      status={status}
      summary={
        <div
          className={[
            'cli-terminal__summary',
            phase === 'complete' ? 'is-visible' : '',
          ]
            .filter(Boolean)
            .join(' ')}
        >
          <i>✓</i>
          <span>{activeScenario.summary[locale]}</span>
        </div>
      }
      surface="hero"
      phase={phase}
      title={activeScenario.command}
      toolbar={
        <nav className="cli-terminal__scenarios" aria-label={ui.scenario}>
          {scenarios.map((scenario, index) => (
            <button
              aria-label={`${scenario.label}: ${scenario.command}`}
              aria-pressed={index === activeIndex}
              className={index === activeIndex ? 'is-active' : undefined}
              data-command={scenario.command}
              data-terminal-scenario={scenario.id}
              key={scenario.id}
              onClick={() => selectScenario(index)}
              title={scenario.command}
              type="button"
            >
              <span>{String(index + 1).padStart(2, '0')}</span>
              {scenario.label}
            </button>
          ))}
        </nav>
      }
    >
      <ol className="cli-terminal__output">
        {activeScenario.output.map((line, index) => (
          <li
            className={[
              `is-${line.tone}`,
              index < visibleLines ? 'is-visible' : '',
            ]
              .filter(Boolean)
              .join(' ')}
            key={`${activeScenario.id}-${line.label.en}`}
          >
            <span>{line.label[locale]}</span>
            <code>{line.value[locale]}</code>
          </li>
        ))}
      </ol>
    </A3sCodeTui>
  );
}
