import {
  ArrowRight,
  CheckCircle2,
  CircleDot,
  Github,
  Layers3,
  Pause,
  Play,
  ShieldCheck,
  Terminal,
} from "lucide-react";
import { Fragment, useEffect, useState, type CSSProperties } from "react";
import { A3SMark } from "./home/a3s-mark";
import { architectureProjects } from "./home/architecture";
import {
  ARCHITECTURE_SELECT_PROJECT_EVENT,
  ArchitectureAtlas,
} from "./home/architecture-atlas";
import { CliTerminalDemo } from "./home/cli-terminal-demo";
import { CopyCommand } from "./home/copy-command";
import { HeroTypewriter } from "./home/hero-typewriter";
import { HomeNav } from "./home/home-nav";
import { homeContent, type AiNativeCopy, type Lang } from "./home/home-content";
import { SiteLink } from "./home/site-link";
import { localePath } from "../lib/i18n";

const signalIcons = [Terminal, ShieldCheck, Layers3, CheckCircle2];

function AiNativeSection({
  content,
  lang,
}: {
  content: AiNativeCopy;
  lang: Lang;
}) {
  const [selectedStepId, setSelectedStepId] = useState(content.steps[0]?.id);
  const [isPlaying, setIsPlaying] = useState(true);
  const selectedStep =
    content.steps.find((step) => step.id === selectedStepId) ??
    content.steps[0];
  const selectedStepIndex = Math.max(
    0,
    content.steps.findIndex((step) => step.id === selectedStep?.id),
  );
  const selectedProjects = selectedStep
    ? selectedStep.projects
        .map((projectId) =>
          architectureProjects.find((project) => project.id === projectId),
        )
        .filter((project) => project !== undefined)
    : [];
  const activePosition =
    content.steps.length > 1
      ? `${(selectedStepIndex / (content.steps.length - 1)) * 100}%`
      : "0%";

  useEffect(() => {
    const reducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)");
    const stopForReducedMotion = () => {
      if (reducedMotion.matches) setIsPlaying(false);
    };

    stopForReducedMotion();
    reducedMotion.addEventListener("change", stopForReducedMotion);
    return () =>
      reducedMotion.removeEventListener("change", stopForReducedMotion);
  }, []);

  useEffect(() => {
    if (!isPlaying) return;

    const timer = window.setInterval(() => {
      setSelectedStepId((currentStepId) => {
        const currentIndex = content.steps.findIndex(
          (step) => step.id === currentStepId,
        );
        return content.steps[(currentIndex + 1) % content.steps.length]?.id;
      });
    }, 3600);

    return () => window.clearInterval(timer);
  }, [content.steps, isPlaying]);

  function openArchitectureProject(projectId: string) {
    window.dispatchEvent(
      new CustomEvent(ARCHITECTURE_SELECT_PROJECT_EVENT, {
        detail: projectId,
      }),
    );
    document.getElementById("architecture")?.scrollIntoView({
      behavior: window.matchMedia("(prefers-reduced-motion: reduce)").matches
        ? "auto"
        : "smooth",
      block: "start",
    });
  }

  return (
    <section
      className="a3s-section a3s-native"
      id="ai-native"
      aria-labelledby="a3s-native-title"
    >
      <div className="a3s-section-heading">
        <div>
          <span className="a3s-section-eyebrow">{content.eyebrow}</span>
          <h2 id="a3s-native-title">{content.title}</h2>
        </div>
        <p>{content.description}</p>
      </div>

      <div className="a3s-native__interaction">
        <header>
          <div>
            <span>{content.interactionEyebrow}</span>
            <h3>{content.interactionTitle}</h3>
            <p>{content.interactionDescription}</p>
          </div>
          <button
            aria-label={isPlaying ? content.pause : content.play}
            className="a3s-native__playback"
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
        </header>
        <div className="a3s-native__flow-viewport">
          <div
            className="a3s-native__flow"
            role="tablist"
            aria-label={content.interactionTitle}
            style={
              {
                gridTemplateColumns: `repeat(${content.steps.length}, minmax(0, 1fr))`,
                minWidth: `${content.steps.length * 176}px`,
              } as CSSProperties
            }
          >
            <span className="a3s-native__rail" aria-hidden="true">
              <i
                className={isPlaying ? "is-playing" : undefined}
                style={
                  {
                    "--active-position": activePosition,
                  } as CSSProperties
                }
              >
                AGENT
              </i>
            </span>
            {content.steps.map((step) => (
              <button
                aria-controls="a3s-native-step-detail"
                aria-selected={step.id === selectedStep?.id}
                className={
                  step.id === selectedStep?.id ? "is-selected" : undefined
                }
                key={step.id}
                onClick={() => {
                  setSelectedStepId(step.id);
                  setIsPlaying(false);
                }}
                role="tab"
                type="button"
              >
                <span>{step.index}</span>
                <b>{step.title}</b>
                <small>{step.summary}</small>
              </button>
            ))}
          </div>
        </div>
        {selectedStep ? (
          <div
            className="a3s-native__detail"
            id="a3s-native-step-detail"
            role="tabpanel"
          >
            <span>{selectedStep.index}</span>
            <div>
              <h4>{selectedStep.title}</h4>
              <p>{selectedStep.detail}</p>
            </div>
            <div className="a3s-native__path" aria-hidden="true">
              {selectedStep.path.map((node, index) => (
                <Fragment key={node}>
                  <code>{node}</code>
                  {index < selectedStep.path.length - 1 ? <i>→</i> : null}
                </Fragment>
              ))}
              {selectedStep.id === "scale" ? (
                <span className="a3s-native__replicas">
                  <i />
                  <i />
                  <i />
                  <b>+N</b>
                </span>
              ) : null}
            </div>
          </div>
        ) : null}
        <div className="a3s-native__stack">
          <header>
            <span>{content.stageProjects}</span>
            <b>
              {String(selectedProjects.length).padStart(2, "0")} /{" "}
              {architectureProjects.length}
            </b>
          </header>
          <div>
            {selectedProjects.map((project) => (
              <button
                key={project.id}
                onClick={() => openArchitectureProject(project.id)}
                type="button"
              >
                <span>{project.name}</span>
                <p>{project.role[lang]}</p>
                <small>{content.openArchitecture} ↘</small>
              </button>
            ))}
          </div>
        </div>
      </div>

      <div className="a3s-native__organization">
        <header>
          <h3>{content.organizationTitle}</h3>
          <p>{content.organizationDescription}</p>
        </header>
        <div>
          {content.reasons.map((reason) => (
            <article key={reason.index}>
              <span>{reason.index}</span>
              <h4>{reason.title}</h4>
              <p>{reason.description}</p>
            </article>
          ))}
        </div>
      </div>
    </section>
  );
}

export default function HomePage({ lang = "cn" }: { lang?: Lang }) {
  const tr = homeContent[lang];

  return (
    <main
      className="a3s-site"
      data-lang={lang}
      lang={lang === "cn" ? "zh-CN" : "en"}
    >
      <HomeNav lang={lang} />

      <section className="a3s-hero" aria-labelledby="a3s-hero-title">
        <div className="a3s-hero__ambient" aria-hidden="true" />
        <div className="a3s-hero__grid">
          <div className="a3s-hero__copy">
            <div className="a3s-eyebrow">
              <span />
              {tr.hero.eyebrow}
            </div>
            <HeroTypewriter
              accent={tr.hero.accent}
              description={tr.hero.description}
              id="a3s-hero-title"
              lineOne={tr.hero.lineOne}
              lineTwo={tr.hero.lineTwo}
            />
            <div className="a3s-hero__actions">
              <SiteLink
                className="a3s-button a3s-button--primary"
                href={localePath("/docs", lang)}
              >
                {tr.hero.primaryAction}
                <ArrowRight aria-hidden="true" />
              </SiteLink>
              <a className="a3s-button a3s-button--ghost" href="#architecture">
                <CircleDot aria-hidden="true" />
                {tr.hero.secondaryAction}
              </a>
            </div>
            <div className="a3s-hero__command" aria-label="A3S launch command">
              <span>$</span>
              <code>a3s code</code>
              <i aria-hidden="true" />
            </div>
          </div>

          <CliTerminalDemo content={tr.hero.terminal} key={lang} />
        </div>
      </section>

      <section className="a3s-signal-strip" aria-label="A3S characteristics">
        <div>
          {tr.signal.map((item, index) => {
            const Icon = signalIcons[index];
            return (
              <span key={item}>
                <Icon aria-hidden="true" />
                {item}
              </span>
            );
          })}
        </div>
      </section>

      <AiNativeSection content={tr.aiNative} lang={lang} />

      <section
        className="a3s-architecture"
        id="architecture"
        aria-labelledby="a3s-architecture-title"
      >
        <div className="a3s-section a3s-architecture__inner">
          <div className="a3s-architecture__copy">
            <div>
              <span className="a3s-section-eyebrow">
                {tr.architecture.eyebrow}
              </span>
              <h2 id="a3s-architecture-title">{tr.architecture.title}</h2>
            </div>
            <p>{tr.architecture.description}</p>
          </div>
          <ArchitectureAtlas lang={lang} />
        </div>
      </section>

      <section
        className="a3s-section a3s-quickstart"
        id="quickstart"
        aria-labelledby="a3s-quickstart-title"
      >
        <div className="a3s-quickstart__copy">
          <span className="a3s-section-eyebrow">{tr.quickstart.eyebrow}</span>
          <h2 id="a3s-quickstart-title">{tr.quickstart.title}</h2>
          <p>{tr.quickstart.description}</p>
          <SiteLink href={localePath("/docs/installation.html", lang)}>
            {tr.quickstart.docs}
            <ArrowRight aria-hidden="true" />
          </SiteLink>
          <small>{tr.quickstart.note}</small>
        </div>
        <div className="a3s-terminal-card">
          <div className="a3s-terminal-card__bar">
            <span>
              <i />
              <i />
              <i />
            </span>
            <code>~/workspace</code>
            <CopyCommand
              command={tr.quickstart.command}
              copyLabel={tr.quickstart.copy}
              copiedLabel={tr.quickstart.copied}
            />
          </div>
          <pre>
            <code>{tr.quickstart.command}</code>
          </pre>
          <div className="a3s-terminal-card__status">
            <span>
              <i /> ready
            </span>
            <span>shell / zsh</span>
          </div>
        </div>
      </section>

      <section className="a3s-cta" aria-labelledby="a3s-cta-title">
        <div className="a3s-cta__mark" aria-hidden="true">
          <A3SMark />
        </div>
        <div className="a3s-cta__copy">
          <span className="a3s-section-eyebrow">{tr.cta.eyebrow}</span>
          <h2 id="a3s-cta-title">{tr.cta.title}</h2>
          <p>{tr.cta.description}</p>
          <div>
            <SiteLink
              className="a3s-button a3s-button--light"
              href={localePath("/docs", lang)}
            >
              {tr.cta.primary}
              <ArrowRight aria-hidden="true" />
            </SiteLink>
            <SiteLink
              className="a3s-button a3s-button--outline"
              href="https://github.com/A3S-Lab/a3s"
              target="_blank"
              rel="noopener noreferrer"
            >
              <Github aria-hidden="true" />
              {tr.cta.secondary}
            </SiteLink>
          </div>
        </div>
      </section>

      <footer className="a3s-footer">
        <div className="a3s-footer__inner">
          <div className="a3s-footer__brand">
            <SiteLink href={localePath("/", lang)}>
              <A3SMark />
              <span>A3S</span>
            </SiteLink>
            <p>{tr.footer.description}</p>
          </div>
          <div className="a3s-footer__column">
            <b>{tr.footer.resources}</b>
            <SiteLink href={localePath("/docs", lang)}>
              {tr.footer.docs}
            </SiteLink>
            <SiteLink href={localePath("/blog", lang)}>
              {tr.footer.blog}
            </SiteLink>
          </div>
          <div className="a3s-footer__column">
            <b>{tr.footer.community}</b>
            <SiteLink
              href="https://github.com/A3S-Lab"
              target="_blank"
              rel="noopener noreferrer"
            >
              {tr.footer.github}
            </SiteLink>
            <SiteLink
              href="https://discord.gg/XVg6Hu6H"
              target="_blank"
              rel="noopener noreferrer"
            >
              {tr.footer.discord}
            </SiteLink>
          </div>
        </div>
        <div className="a3s-footer__base">
          <span>© {new Date().getFullYear()} A3S Lab</span>
          <span>{tr.footer.license}</span>
          <span>RUST / ASYNC / OPEN</span>
        </div>
      </footer>
    </main>
  );
}
