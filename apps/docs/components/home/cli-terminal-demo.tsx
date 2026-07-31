import { Pause, Play } from "lucide-react";
import { useEffect, useRef, useState, type KeyboardEvent } from "react";
import type { CliTerminalCopy } from "./home-content";

const TYPE_START_DELAY = 360;
const OUTPUT_START_DELAY = 420;
const OUTPUT_LINE_DELAY = 190;
const COMMAND_HOLD_DELAY = 1800;

export function CliTerminalDemo({ content }: { content: CliTerminalCopy }) {
  const [activeIndex, setActiveIndex] = useState(0);
  const [typedLength, setTypedLength] = useState(0);
  const [visibleLineCount, setVisibleLineCount] = useState(0);
  const [isPlaying, setIsPlaying] = useState(true);
  const [reducedMotion, setReducedMotion] = useState(false);
  const tabRefs = useRef<Array<HTMLButtonElement | null>>([]);
  const command =
    content.commands[activeIndex] ?? content.commands[0] ?? undefined;
  const commandTyped = command ? typedLength >= command.command.length : false;
  const commandComplete = command
    ? commandTyped && visibleLineCount >= command.output.length
    : false;
  const phase =
    !isPlaying && !commandComplete
      ? "paused"
      : !commandTyped
        ? "typing"
        : !commandComplete
          ? "running"
          : "complete";
  const phaseLabel = content[phase];

  useEffect(() => {
    const motionPreference = window.matchMedia(
      "(prefers-reduced-motion: reduce)",
    );
    const updatePreference = () => setReducedMotion(motionPreference.matches);

    updatePreference();
    motionPreference.addEventListener("change", updatePreference);
    return () =>
      motionPreference.removeEventListener("change", updatePreference);
  }, []);

  useEffect(() => {
    if (!reducedMotion || !command) return;
    setIsPlaying(false);
    setTypedLength(command.command.length);
    setVisibleLineCount(command.output.length);
  }, [command, reducedMotion]);

  useEffect(() => {
    if (!command || !isPlaying || reducedMotion) return;

    let delay = TYPE_START_DELAY;
    let advance = () => {
      setTypedLength((length) => Math.min(command.command.length, length + 2));
    };

    if (typedLength > 0 && !commandTyped) {
      const nextCharacters = command.command.slice(
        typedLength,
        typedLength + 2,
      );
      delay = nextCharacters.includes(" ") ? 58 : 34;
    } else if (commandTyped && !commandComplete) {
      delay = visibleLineCount === 0 ? OUTPUT_START_DELAY : OUTPUT_LINE_DELAY;
      advance = () => {
        setVisibleLineCount((count) =>
          Math.min(command.output.length, count + 1),
        );
      };
    } else if (commandComplete) {
      delay = COMMAND_HOLD_DELAY;
      advance = () => {
        setActiveIndex((index) => (index + 1) % content.commands.length);
        setTypedLength(0);
        setVisibleLineCount(0);
      };
    }

    const timer = window.setTimeout(advance, delay);
    return () => window.clearTimeout(timer);
  }, [
    command,
    commandComplete,
    commandTyped,
    content.commands.length,
    isPlaying,
    reducedMotion,
    typedLength,
    visibleLineCount,
  ]);

  if (!command) return null;

  function selectCommand(index: number) {
    const nextCommand = content.commands[index];
    if (!nextCommand) return;

    setActiveIndex(index);
    if (reducedMotion) {
      setTypedLength(nextCommand.command.length);
      setVisibleLineCount(nextCommand.output.length);
      setIsPlaying(false);
    } else {
      setTypedLength(0);
      setVisibleLineCount(0);
      setIsPlaying(true);
    }
  }

  function handleTabKeyDown(
    event: KeyboardEvent<HTMLButtonElement>,
    index: number,
  ) {
    let nextIndex: number | undefined;
    if (event.key === "ArrowRight" || event.key === "ArrowDown") {
      nextIndex = (index + 1) % content.commands.length;
    } else if (event.key === "ArrowLeft" || event.key === "ArrowUp") {
      nextIndex =
        (index - 1 + content.commands.length) % content.commands.length;
    } else if (event.key === "Home") {
      nextIndex = 0;
    } else if (event.key === "End") {
      nextIndex = content.commands.length - 1;
    }

    if (nextIndex === undefined) return;
    event.preventDefault();
    selectCommand(nextIndex);
    tabRefs.current[nextIndex]?.focus();
  }

  const liveMessage = commandComplete
    ? `${command.command}. ${command.summary}`
    : `${command.command}. ${phaseLabel}`;

  return (
    <div
      aria-label={content.ariaLabel}
      className="a3s-system-panel a3s-cli-terminal"
      data-phase={phase}
    >
      <div className="a3s-system-panel__chrome">
        <span aria-hidden="true">
          <i /> <i /> <i />
        </span>
        <code>{content.title}</code>
        <b>
          <i aria-hidden="true" />
          {phaseLabel}
        </b>
      </div>

      <div
        aria-label={content.ariaLabel}
        className="a3s-cli-terminal__tabs"
        role="tablist"
      >
        {content.commands.map((item, index) => (
          <button
            aria-label={`${item.label}: ${item.command}`}
            aria-controls="a3s-cli-terminal-screen"
            aria-selected={index === activeIndex}
            key={item.id}
            onClick={() => selectCommand(index)}
            onKeyDown={(event) => handleTabKeyDown(event, index)}
            ref={(node) => {
              tabRefs.current[index] = node;
            }}
            role="tab"
            tabIndex={index === activeIndex ? 0 : -1}
            type="button"
          >
            <small>{String(index + 1).padStart(2, "0")}</small>
            <span>{item.label}</span>
          </button>
        ))}
      </div>

      <div
        aria-label={command.command}
        className="a3s-cli-terminal__screen"
        id="a3s-cli-terminal-screen"
        role="tabpanel"
      >
        <div className="a3s-cli-terminal__prompt" aria-hidden="true">
          <span>$</span>
          <code>{command.command.slice(0, typedLength)}</code>
          {!commandTyped ? <i /> : null}
        </div>
        <div className="a3s-cli-terminal__output" aria-hidden="true">
          {command.output.slice(0, visibleLineCount).map((line, index) => (
            <div
              className={`a3s-cli-terminal__line a3s-cli-terminal__line--${line.tone ?? "default"}`}
              key={`${command.id}-${index}-${line.text}`}
            >
              {line.tone === "command" ? <span>$</span> : null}
              <code>{line.text}</code>
            </div>
          ))}
          {commandTyped && !commandComplete ? (
            <div className="a3s-cli-terminal__activity">
              <span>
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

      <div className="a3s-cli-terminal__footer">
        <span className="a3s-cli-terminal__state">
          <i aria-hidden="true" />
          {phaseLabel}
        </span>
        <span className="a3s-cli-terminal__summary">{command.summary}</span>
        <span className="a3s-cli-terminal__progress" aria-hidden="true">
          {content.commands.map((item, index) => (
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
  );
}
