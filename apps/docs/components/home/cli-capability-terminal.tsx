'use client';

import { useEffect, useRef, useState } from 'react';
import {
  cliCapabilityScenes,
  cliTerminalCopy,
} from '@/components/home/cli-capability-terminal-data';
import type { Lang } from '@/components/home/home-content';

const sceneDurationMs = 5_600;
const characterDelayMs = 24;

export function CliCapabilityTerminal({ lang }: { lang: Lang }) {
  const [activeIndex, setActiveIndex] = useState(0);
  const [hasAdvanced, setHasAdvanced] = useState(false);
  const [isRunning, setIsRunning] = useState(false);
  const [typedCommand, setTypedCommand] = useState(cliCapabilityScenes[0].command[lang]);
  const [visibleOutputCount, setVisibleOutputCount] = useState(cliCapabilityScenes[0].output.length);
  const terminalRef = useRef<HTMLElement>(null);
  const tr = cliTerminalCopy[lang];
  const activeScene = cliCapabilityScenes[activeIndex];
  const previousScene = hasAdvanced
    ? cliCapabilityScenes[(activeIndex - 1 + cliCapabilityScenes.length) % cliCapabilityScenes.length]
    : undefined;

  useEffect(() => {
    const terminal = terminalRef.current;
    if (!terminal) return undefined;

    const reducedMotion = window.matchMedia('(prefers-reduced-motion: reduce)');
    let isIntersecting = false;

    const syncPlayback = () => {
      setIsRunning(
        isIntersecting
        && document.visibilityState === 'visible'
        && !reducedMotion.matches,
      );
    };
    const observer = new IntersectionObserver(([entry]) => {
      isIntersecting = entry.isIntersecting;
      syncPlayback();
    }, { threshold: 0.28 });

    observer.observe(terminal);
    document.addEventListener('visibilitychange', syncPlayback);
    reducedMotion.addEventListener('change', syncPlayback);

    return () => {
      observer.disconnect();
      document.removeEventListener('visibilitychange', syncPlayback);
      reducedMotion.removeEventListener('change', syncPlayback);
    };
  }, []);

  useEffect(() => {
    const command = activeScene.command[lang];

    if (!isRunning) {
      setTypedCommand(command);
      setVisibleOutputCount(activeScene.output.length);
      return undefined;
    }

    let typedCharacters = 0;
    setTypedCommand('');
    setVisibleOutputCount(0);

    const typingTimer = window.setInterval(() => {
      typedCharacters += 1;
      setTypedCommand(command.slice(0, typedCharacters));
      if (typedCharacters >= command.length) window.clearInterval(typingTimer);
    }, characterDelayMs);
    const typingDuration = command.length * characterDelayMs;
    const outputTimers = activeScene.output.map((_, index) => window.setTimeout(
      () => setVisibleOutputCount(index + 1),
      typingDuration + 240 + (index * 300),
    ));
    const nextSceneTimer = window.setTimeout(() => {
      setHasAdvanced(true);
      setActiveIndex((current) => (current + 1) % cliCapabilityScenes.length);
    }, sceneDurationMs);

    return () => {
      window.clearInterval(typingTimer);
      outputTimers.forEach((timer) => window.clearTimeout(timer));
      window.clearTimeout(nextSceneTimer);
    };
  }, [activeScene, isRunning, lang]);

  return (
    <figure
      className="a3s-cli-terminal"
      data-running={isRunning ? 'true' : 'false'}
      aria-label={tr.accessibleLabel}
      ref={terminalRef}
      role="img"
    >
      <figcaption hidden>{tr.accessibleLabel}</figcaption>
      <div aria-hidden="true">
        <div className="a3s-cli-terminal__chrome">
          <span className="a3s-cli-terminal__dots"><i /><i /><i /></span>
          <code>{tr.chromeTitle}</code>
          <b><i /> {tr.ready}</b>
        </div>
        <div className="a3s-cli-terminal__body">
          <div className="a3s-cli-terminal__screen">
            <div className="a3s-cli-terminal__motd">
              <span>Last login: Fri Aug 07 10:34:08 on ttys001</span>
              <strong>{tr.motd}</strong>
              <span>{tr.hint}</span>
            </div>
            {previousScene ? (
              <div className="a3s-cli-terminal__history">
                <div className="a3s-cli-terminal__prompt is-muted">
                  <span><b>a3s</b><i>@</i><b>local</b><i>:</i><em>~/workspace</em><strong>$</strong></span>
                  <code>{previousScene.command[lang]}</code>
                </div>
                <p><b>✓</b> {previousScene.output[previousScene.output.length - 1].value[lang]}</p>
              </div>
            ) : null}
            <section className="a3s-cli-terminal__session" key={`${lang}-${activeScene.id}`}>
              <div className="a3s-cli-terminal__prompt">
                <span><b>a3s</b><i>@</i><b>local</b><i>:</i><em>~/workspace</em><strong>$</strong></span>
                <code>{typedCommand}</code>
                <i className="a3s-cli-terminal__caret" />
              </div>
              <ol className="a3s-cli-terminal__output">
                {activeScene.output.map((row, rowIndex) => (
                  <li
                    data-tone={row.tone}
                    data-visible={rowIndex < visibleOutputCount ? 'true' : 'false'}
                    key={row.key}
                  >
                    <code>[{row.key}]</code>
                    <span>{row.value[lang]}</span>
                  </li>
                ))}
              </ol>
              <p className="a3s-cli-terminal__complete" data-visible={visibleOutputCount === activeScene.output.length ? 'true' : 'false'}>
                <b>✓</b> command completed successfully
              </p>
            </section>
            <ul hidden>
              {cliCapabilityScenes.map((scene) => <li key={scene.id}>{scene.command[lang]}</li>)}
            </ul>
          </div>
          <footer className="a3s-cli-terminal__statusline">
            <b>{activeScene.label}</b>
            <span>{tr.capability} {String(activeIndex + 1).padStart(2, '0')}/{String(cliCapabilityScenes.length).padStart(2, '0')}</span>
            <div className="a3s-cli-terminal__progress"><i key={activeIndex} /></div>
            <code>UTF-8 · zsh</code>
          </footer>
        </div>
      </div>
    </figure>
  );
}
