"use client";

import { Pause, Play } from "lucide-react";
import { useEffect, useRef, useState, type KeyboardEvent } from "react";
import { CanvasSignalField } from "./canvas-signal-field";
import type { CloudLifecycleCopy } from "./home-content";

const TYPE_START_DELAY = 360;
const OUTPUT_START_DELAY = 420;
const OUTPUT_LINE_DELAY = 230;
const STAGE_HOLD_DELAY = 2200;

export function CloudLifecycleTerminal({
  content,
}: {
  content: CloudLifecycleCopy;
}) {
  const [activeIndex, setActiveIndex] = useState(0);
  const [typedLength, setTypedLength] = useState(0);
  const [visibleLineCount, setVisibleLineCount] = useState(0);
  const [isPlaying, setIsPlaying] = useState(true);
  const [reducedMotion, setReducedMotion] = useState(false);
  const stageTabListRef = useRef<HTMLDivElement | null>(null);
  const stageTabRefs = useRef<Array<HTMLButtonElement | null>>([]);
  const stage = content.stages[activeIndex] ?? content.stages[0];
  const commandTyped = stage ? typedLength >= stage.command.length : false;
  const stageComplete = stage
    ? commandTyped && visibleLineCount >= stage.lines.length
    : false;
  const phase =
    !isPlaying && !stageComplete
      ? "paused"
      : !commandTyped
        ? "typing"
        : !stageComplete
          ? "running"
          : "complete";
  const phaseLabel = content[phase];
  const activeSystems = content.systems.flatMap((system, index) =>
    stage?.systems.includes(system.id) ? [index] : [],
  );

  useEffect(() => {
    const motionPreference = window.matchMedia(
      "(prefers-reduced-motion: reduce)",
    );
    const updatePreference = () => {
      setReducedMotion(motionPreference.matches);
    };

    updatePreference();
    motionPreference.addEventListener("change", updatePreference);
    return () =>
      motionPreference.removeEventListener("change", updatePreference);
  }, []);

  useEffect(() => {
    if (!reducedMotion || content.stages.length === 0) return;

    const lastIndex = content.stages.length - 1;
    const lastStage = content.stages[lastIndex];
    setActiveIndex(lastIndex);
    setTypedLength(lastStage?.command.length ?? 0);
    setVisibleLineCount(lastStage?.lines.length ?? 0);
    setIsPlaying(false);
  }, [content.stages, reducedMotion]);

  useEffect(() => {
    if (!stage || !isPlaying || reducedMotion) return;

    let delay = TYPE_START_DELAY;
    let advance = () => {
      setTypedLength((length) => Math.min(stage.command.length, length + 3));
    };

    if (typedLength > 0 && !commandTyped) {
      const nextCharacters = stage.command.slice(typedLength, typedLength + 3);
      delay = nextCharacters.includes(" ") ? 62 : 36;
    } else if (commandTyped && !stageComplete) {
      delay = visibleLineCount === 0 ? OUTPUT_START_DELAY : OUTPUT_LINE_DELAY;
      advance = () => {
        setVisibleLineCount((count) => Math.min(stage.lines.length, count + 1));
      };
    } else if (stageComplete) {
      delay = STAGE_HOLD_DELAY;
      advance = () => {
        setActiveIndex((index) => (index + 1) % content.stages.length);
        setTypedLength(0);
        setVisibleLineCount(0);
      };
    }

    const timer = window.setTimeout(advance, delay);
    return () => window.clearTimeout(timer);
  }, [
    commandTyped,
    content.stages.length,
    isPlaying,
    reducedMotion,
    stage,
    stageComplete,
    typedLength,
    visibleLineCount,
  ]);

  useEffect(() => {
    const tabList = stageTabListRef.current;
    const activeTab = stageTabRefs.current[activeIndex];
    if (!tabList || !activeTab || tabList.scrollWidth <= tabList.clientWidth) {
      return;
    }

    const centeredLeft =
      activeTab.offsetLeft - (tabList.clientWidth - activeTab.offsetWidth) / 2;
    tabList.scrollTo({
      behavior: reducedMotion ? "auto" : "smooth",
      left: Math.max(0, centeredLeft),
    });
  }, [activeIndex, reducedMotion]);

  if (!stage) return null;

  function selectStage(index: number) {
    const nextStage = content.stages[index];
    if (!nextStage) return;

    setActiveIndex(index);
    if (reducedMotion) {
      setTypedLength(nextStage.command.length);
      setVisibleLineCount(nextStage.lines.length);
      setIsPlaying(false);
      return;
    }

    setTypedLength(0);
    setVisibleLineCount(0);
    setIsPlaying(true);
  }

  function handleStageKeyDown(
    event: KeyboardEvent<HTMLButtonElement>,
    index: number,
  ) {
    let nextIndex: number | undefined;
    if (event.key === "ArrowRight" || event.key === "ArrowDown") {
      nextIndex = (index + 1) % content.stages.length;
    } else if (event.key === "ArrowLeft" || event.key === "ArrowUp") {
      nextIndex = (index - 1 + content.stages.length) % content.stages.length;
    } else if (event.key === "Home") {
      nextIndex = 0;
    } else if (event.key === "End") {
      nextIndex = content.stages.length - 1;
    }

    if (nextIndex === undefined) return;
    event.preventDefault();
    selectStage(nextIndex);
    stageTabRefs.current[nextIndex]?.focus();
  }

  const liveMessage = stageComplete ? `${stage.title}. ${stage.result}` : "";

  return (
    <section
      className="a3s-cloud-lifecycle"
      id="cloud-lifecycle"
      aria-labelledby="a3s-cloud-lifecycle-title"
    >
      <div className="a3s-section a3s-cloud-lifecycle__inner">
        <div className="a3s-section-heading">
          <div>
            <span className="a3s-section-eyebrow">{content.eyebrow}</span>
            <h2 id="a3s-cloud-lifecycle-title">{content.title}</h2>
          </div>
          <p>{content.description}</p>
        </div>

        <dl className="a3s-cloud-lifecycle__contract">
          {content.contract.map((item) => (
            <div key={item.label}>
              <dt>{item.label}</dt>
              <dd>
                <code>{item.value}</code>
              </dd>
            </div>
          ))}
        </dl>

        <div
          aria-label={content.ariaLabel}
          className="a3s-cloud-terminal"
          data-phase={phase}
        >
          <div className="a3s-cloud-terminal__chrome">
            <span aria-hidden="true">
              <i />
              <i />
              <i />
            </span>
            <code>{content.terminalTitle}</code>
            <b>
              <i aria-hidden="true" />
              {phaseLabel}
            </b>
          </div>

          <div className="a3s-cloud-terminal__body">
            <div
              aria-label={content.stageNavigation}
              className="a3s-cloud-terminal__stages"
              ref={stageTabListRef}
              role="tablist"
            >
              {content.stages.map((item, index) => (
                <button
                  aria-label={`${item.phase}: ${item.title}. ${item.command}`}
                  aria-controls="a3s-cloud-terminal-screen"
                  aria-selected={index === activeIndex}
                  id={`a3s-cloud-stage-${item.id}`}
                  key={item.id}
                  onClick={() => selectStage(index)}
                  onKeyDown={(event) => handleStageKeyDown(event, index)}
                  ref={(node) => {
                    stageTabRefs.current[index] = node;
                  }}
                  role="tab"
                  tabIndex={index === activeIndex ? 0 : -1}
                  type="button"
                >
                  <small>
                    {item.index} / {item.phase}
                  </small>
                  <strong>{item.title}</strong>
                  <span>{item.summary}</span>
                </button>
              ))}
            </div>

            <div className="a3s-cloud-terminal__workspace">
              <div
                aria-busy={isPlaying && !stageComplete}
                aria-labelledby={`a3s-cloud-stage-${stage.id}`}
                className="a3s-cloud-terminal__screen"
                id="a3s-cloud-terminal-screen"
                role="tabpanel"
              >
                <CanvasSignalField
                  activeIndex={activeIndex}
                  activeSystems={activeSystems}
                  playing={isPlaying}
                  variant="terminal"
                />
                <header>
                  <span>{stage.phase}</span>
                  <b>{stage.title}</b>
                  <small>{stage.summary}</small>
                </header>

                <div className="a3s-cloud-terminal__prompt" aria-hidden="true">
                  <span>{stage.prompt}</span>
                  <code>{stage.command.slice(0, typedLength)}</code>
                  {!commandTyped ? <i /> : null}
                </div>

                <div className="a3s-cloud-terminal__output" aria-hidden="true">
                  {stage.lines.slice(0, visibleLineCount).map((line, index) => (
                    <div
                      className={`a3s-cloud-terminal__line a3s-cloud-terminal__line--${line.tone ?? "default"}`}
                      key={`${stage.id}-${index}-${line.source}-${line.text}`}
                    >
                      <span>{line.source}</span>
                      <code>{line.text}</code>
                    </div>
                  ))}
                  {commandTyped && !stageComplete ? (
                    <div className="a3s-cloud-terminal__activity">
                      <span aria-hidden="true">
                        <i />
                        <i />
                        <i />
                      </span>
                      {phaseLabel}
                    </div>
                  ) : null}
                </div>

                <p className="a3s-visually-hidden" aria-live="polite">
                  {liveMessage}
                </p>
              </div>

              <ol
                aria-label={content.systemPath}
                className="a3s-cloud-terminal__systems"
              >
                {content.systems.map((system, index) => {
                  const active = stage.systems.includes(system.id);
                  return (
                    <li
                      className={active ? "is-active" : undefined}
                      key={system.id}
                    >
                      <span>{String(index + 1).padStart(2, "0")}</span>
                      <div>
                        <b>{system.label}</b>
                        <small>{system.detail}</small>
                      </div>
                    </li>
                  );
                })}
              </ol>
            </div>
          </div>

          <div className="a3s-cloud-terminal__footer">
            <span>
              {stage.index} / {String(content.stages.length).padStart(2, "0")}
            </span>
            <p>{stage.result}</p>
            <span className="a3s-cloud-terminal__progress" aria-hidden="true">
              {content.stages.map((item, index) => (
                <i
                  className={index === activeIndex ? "is-active" : undefined}
                  key={item.id}
                />
              ))}
            </span>
            <button
              aria-label={isPlaying ? content.pause : content.play}
              disabled={reducedMotion}
              onClick={() => setIsPlaying((value) => !value)}
              type="button"
            >
              {isPlaying ? (
                <Pause aria-hidden="true" />
              ) : (
                <Play aria-hidden="true" />
              )}
              <span>{isPlaying ? content.pause : content.play}</span>
            </button>
          </div>
        </div>

        <div className="a3s-cloud-lifecycle__delivery">
          <div>
            <span>{content.verified.label}</span>
            <p>{content.verified.value}</p>
          </div>
          <div>
            <span>{content.next.label}</span>
            <p>{content.next.value}</p>
          </div>
        </div>
      </div>
    </section>
  );
}
