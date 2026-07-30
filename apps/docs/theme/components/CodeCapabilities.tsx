import { withBase } from '@rspress/core/runtime';
import type { CSSProperties, ReactNode } from 'react';
import { A3sCodeTui, type A3sCodeTuiSurface } from './A3sCodeTui';
import {
  codeCapabilityContent,
  type CapabilityLocale,
} from './code-capability-content';
import { CapabilityIcon, type IconName } from './CapabilityIcon';

function ArrowIcon() {
  return (
    <svg aria-hidden="true" viewBox="0 0 20 20">
      <path d="M4 10h11m-4-4 4 4-4 4" />
    </svg>
  );
}

function CapabilityMeta({
  icon,
  index,
  label,
  status,
}: {
  icon: IconName;
  index: string;
  label: string;
  status: string;
}) {
  return (
    <div className="code-capability-meta">
      <span>{index}</span>
      <CapabilityIcon name={icon} />
      <b>{label}</b>
      <small>
        <i /> {status}
      </small>
    </div>
  );
}

function CapabilityLink({
  action,
  href,
  locale,
}: {
  action: string;
  href: string;
  locale: CapabilityLocale;
}) {
  const localizedPath = locale === 'en' ? `/en${href}` : href;

  return (
    <a className="code-capability-link" href={withBase(localizedPath)}>
      {action}
      <ArrowIcon />
    </a>
  );
}

function TuiModeBar() {
  return (
    <div className="cli-terminal__scenarios a3s-code-tui__modes">
      <span className="is-active">
        <small>01</small>
        DEFAULT
      </span>
      <span>
        <small>02</small>
        GPT-5.6
      </span>
      <span>
        <small>03</small>
        EFFORT HIGH
      </span>
    </div>
  );
}

function TuiMetrics() {
  return (
    <span className="a3s-code-tui__metrics">
      <b>↑ 0</b>
      <b>↓ 0</b>
    </span>
  );
}

function CapabilityTuiDemo({
  children,
  className,
  footer,
  prompt,
  status,
  summary,
  summaryIcon = 'check',
  surface,
}: {
  children: ReactNode;
  className: string;
  footer: string;
  prompt: string;
  status: string;
  summary: string;
  summaryIcon?: IconName;
  surface: Exclude<A3sCodeTuiSurface, 'hero'>;
}) {
  return (
    <A3sCodeTui
      ariaHidden
      className={`code-capability-demo ${className}`}
      footerAction={<TuiMetrics />}
      footerLead="DEFAULT"
      footerMeta={footer}
      prompt={prompt}
      promptActive
      status={status}
      summary={
        <div className="a3s-code-tui__result">
          <CapabilityIcon name={summaryIcon} />
          <span>{summary}</span>
        </div>
      }
      surface={surface}
      title="a3s code · ~/workspace"
      toolbar={<TuiModeBar />}
    >
      {children}
    </A3sCodeTui>
  );
}

export function CodeCapabilitiesMarkdown({
  locale,
}: {
  locale: CapabilityLocale;
}) {
  const content = codeCapabilityContent[locale];
  const capabilities = [
    content.intelligence,
    content.hitl,
    content.progressive,
    content.runtime,
    content.context,
  ];

  return (
    <section>
      <h2>{content.title}</h2>
      <p>{content.description}</p>
      {capabilities.map((capability) => (
        <article key={capability.index}>
          <h3>{capability.title}</h3>
          <p>{capability.description}</p>
        </article>
      ))}
    </section>
  );
}

export function CodeCapabilities({ locale }: { locale: CapabilityLocale }) {
  const content = codeCapabilityContent[locale];

  return (
    <section
      className="code-capabilities"
      id="capabilities"
      aria-labelledby="code-capabilities-title"
    >
      <div className="code-capabilities__inner">
        <header className="code-capabilities__heading">
          <div>
            <span>{content.eyebrow}</span>
            <h2 id="code-capabilities-title">{content.title}</h2>
          </div>
          <p>{content.description}</p>
        </header>

        <div className="code-capability-grid">
          <article className="code-capability-card is-intelligence">
            <div className="code-capability-card__copy">
              <CapabilityMeta
                icon="code"
                index={content.intelligence.index}
                label={content.intelligence.label}
                status={content.intelligence.status}
              />
              <h3>{content.intelligence.title}</h3>
              <p>{content.intelligence.description}</p>
              <CapabilityLink
                action={content.action}
                href={content.intelligence.href}
                locale={locale}
              />
            </div>

            <CapabilityTuiDemo
              className="code-intelligence-demo"
              footer={content.intelligence.evidence}
              prompt={content.intelligence.command}
              status="LSP READY"
              summary={content.intelligence.evidence}
              surface="code-intelligence"
            >
              <div className="code-intelligence-demo__request">
                <CapabilityIcon name="search" />
                <div>
                  <small>code_navigation</small>
                  <b>definition · saved file</b>
                </div>
                <em>RUNNING</em>
              </div>
              <div className="code-intelligence-demo__result">
                <span>└</span>
                <div>
                  <b>{content.intelligence.symbol}</b>
                  <code>{content.intelligence.location}</code>
                </div>
                <CapabilityIcon name="check" />
              </div>
            </CapabilityTuiDemo>
          </article>

          <article className="code-capability-card is-hitl">
            <div className="code-capability-card__copy">
              <CapabilityMeta
                icon="shield"
                index={content.hitl.index}
                label={content.hitl.label}
                status={content.hitl.status}
              />
              <h3>{content.hitl.title}</h3>
              <p>{content.hitl.description}</p>
              <CapabilityLink
                action={content.action}
                href={content.hitl.href}
                locale={locale}
              />
            </div>

            <CapabilityTuiDemo
              className="code-hitl-demo"
              footer={content.hitl.risk}
              prompt={content.hitl.command}
              status="ASK"
              summary={content.hitl.outcome}
              surface="hitl"
            >
              <div className="code-hitl-demo__title">
                <CapabilityIcon name="shield" />
                <b>{content.hitl.prompt}</b>
                <span>ASK</span>
              </div>
              <div className="code-hitl-demo__call">
                <span>Run</span>
                <code>{content.hitl.run}</code>
                <small>{content.hitl.risk}</small>
              </div>
              <div className="code-hitl-demo__options">
                {content.hitl.options.map((option, index) => (
                  <div
                    key={option}
                    style={{ '--option-index': index } as CSSProperties}
                  >
                    <span>{index + 1}</span>
                    <i>
                      {index === 0
                        ? '↵'
                        : index === 2
                          ? '⌘'
                          : index === 3
                            ? '⊘'
                            : '◎'}
                    </i>
                    <b>{option}</b>
                  </div>
                ))}
              </div>
            </CapabilityTuiDemo>
          </article>

          <article className="code-capability-card is-progressive">
            <div className="code-capability-card__copy">
              <CapabilityMeta
                icon="spark"
                index={content.progressive.index}
                label={content.progressive.label}
                status={content.progressive.status}
              />
              <h3>{content.progressive.title}</h3>
              <p>{content.progressive.description}</p>
              <CapabilityLink
                action={content.action}
                href={content.progressive.href}
                locale={locale}
              />
            </div>

            <CapabilityTuiDemo
              className="code-progressive-demo"
              footer={content.progressive.registry}
              prompt={content.progressive.prompt}
              status="DISCLOSING"
              summary={content.progressive.result}
              summaryIcon="spark"
              surface="progressive-api"
            >
              <div className="code-progressive-demo__registry">
                <div>
                  <CapabilityIcon name="database" />
                  <span>{content.progressive.registry}</span>
                </div>
                {content.progressive.rows.map(([name, state], index) => (
                  <div
                    className="code-progressive-demo__row"
                    key={name}
                    style={{ '--api-index': index } as CSSProperties}
                  >
                    <code>{name}</code>
                    <i />
                    <b>{state}</b>
                  </div>
                ))}
              </div>
            </CapabilityTuiDemo>
          </article>

          <article className="code-capability-card is-runtime">
            <div className="code-capability-card__copy">
              <CapabilityMeta
                icon="runtime"
                index={content.runtime.index}
                label={content.runtime.label}
                status={content.runtime.status}
              />
              <h3>{content.runtime.title}</h3>
              <p>{content.runtime.description}</p>
              <CapabilityLink
                action={content.action}
                href={content.runtime.href}
                locale={locale}
              />
            </div>

            <CapabilityTuiDemo
              className="code-runtime-demo"
              footer={content.runtime.footer}
              prompt={content.runtime.call}
              status="STREAMING"
              summary={content.runtime.complete}
              surface="runtime-tool"
            >
              <div className="code-runtime-demo__gate">
                <span>✓</span>
                <code>{content.runtime.gate}</code>
              </div>
              <div className="code-runtime-demo__submitted">
                <CapabilityIcon name="branch" />
                {content.runtime.submitted}
              </div>
              <div className="code-runtime-demo__tasks">
                {content.runtime.tasks.map((task, index) => (
                  <div
                    key={task}
                    style={{ '--task-index': index } as CSSProperties}
                  >
                    <span>{String(index + 1).padStart(2, '0')}</span>
                    <b>{task}</b>
                    <i />
                    <small className="is-queued">queued</small>
                    <small className="is-running">running</small>
                    <small className="is-done">done</small>
                  </div>
                ))}
              </div>
            </CapabilityTuiDemo>
          </article>

          <article className="code-capability-card is-context">
            <div className="code-capability-card__copy">
              <CapabilityMeta
                icon="history"
                index={content.context.index}
                label={content.context.label}
                status={content.context.status}
              />
              <h3>{content.context.title}</h3>
              <p>{content.context.description}</p>
              <CapabilityLink
                action={content.action}
                href={content.context.href}
                locale={locale}
              />
            </div>

            <CapabilityTuiDemo
              className="code-context-demo"
              footer={content.context.safety}
              prompt={content.context.query}
              status="RECALL"
              summary={content.context.safety}
              summaryIcon="shield"
              surface="cross-session-context"
            >
              <div className="code-context-demo__hits">
                {content.context.hits.map(
                  ([index, meta, title, snippet], hitIndex) => (
                    <div
                      key={index}
                      style={{ '--hit-index': hitIndex } as CSSProperties}
                    >
                      <span>{index}</span>
                      <p>
                        <small>{meta}</small>
                        <b>{title}</b>
                        <em>{snippet}</em>
                      </p>
                    </div>
                  ),
                )}
              </div>
              <div className="code-context-demo__actions">
                <span>
                  <code>{content.context.attach}</code>
                  <b>{content.context.staged}</b>
                </span>
                <span>
                  <code>{content.context.save}</code>
                  <b>{content.context.saved}</b>
                </span>
              </div>
            </CapabilityTuiDemo>
          </article>
        </div>
      </div>
    </section>
  );
}
