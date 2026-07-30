import { withBase } from '@rspress/core/runtime';
import type { CSSProperties } from 'react';
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

            <div
              className="code-capability-demo code-intelligence-demo"
              aria-hidden="true"
            >
              <div className="code-capability-demo__chrome">
                <span>
                  <i />
                  <i />
                  <i />
                </span>
                <code>{content.intelligence.windowTitle}</code>
                <b>
                  <i /> LSP READY
                </b>
              </div>
              <div className="code-intelligence-demo__tabs">
                {content.intelligence.tabs.map((tab, index) => (
                  <span
                    className={index === 1 ? 'is-active' : undefined}
                    key={tab}
                  >
                    {tab}
                  </span>
                ))}
              </div>
              <div className="code-intelligence-demo__body">
                <div className="code-intelligence-demo__gutter">
                  <span>146</span>
                  <span>147</span>
                  <span>148</span>
                  <span>149</span>
                </div>
                <pre>
                  <code>
                    <span>
                      pub struct <b>{content.intelligence.symbol}</b> {'{'}
                    </span>
                    {'\n'}
                    <span> workspace: Arc&lt;WorkspaceServices&gt;,</span>
                    {'\n'}
                    <span> tools: Arc&lt;ToolExecutor&gt;,</span>
                    {'\n'}
                    <span>{'}'}</span>
                  </code>
                </pre>
                <div className="code-intelligence-demo__cursor" />
              </div>
              <div className="code-intelligence-demo__command">
                <span>›</span>
                <code>{content.intelligence.command}</code>
                <i />
              </div>
              <div className="code-intelligence-demo__result">
                <CapabilityIcon name="search" />
                <div>
                  <b>{content.intelligence.symbol}</b>
                  <code>{content.intelligence.location}</code>
                </div>
                <CapabilityIcon name="check" />
              </div>
              <div className="code-intelligence-demo__evidence">
                {content.intelligence.evidence}
              </div>
            </div>
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

            <div
              className="code-capability-demo code-hitl-demo"
              aria-hidden="true"
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
              <div className="code-hitl-demo__outcome">
                <CapabilityIcon name="check" />
                {content.hitl.outcome}
              </div>
            </div>
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

            <div
              className="code-capability-demo code-progressive-demo"
              aria-hidden="true"
            >
              <div className="code-progressive-demo__prompt">
                <CapabilityIcon name="spark" />
                <span>TURN INTENT</span>
                <b>{content.progressive.prompt}</b>
              </div>
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
              <div className="code-progressive-demo__result">
                <span>
                  <i />
                </span>
                {content.progressive.result}
              </div>
            </div>
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

            <div
              className="code-capability-demo code-runtime-demo"
              aria-hidden="true"
            >
              <div className="code-capability-demo__chrome">
                <span>
                  <i />
                  <i />
                  <i />
                </span>
                <code>{content.runtime.windowTitle}</code>
                <b>
                  <i /> STREAMING
                </b>
              </div>
              <div className="code-runtime-demo__gate">
                <span>✓</span>
                <code>{content.runtime.gate}</code>
              </div>
              <div className="code-runtime-demo__call">
                <CapabilityIcon name="runtime" />
                <code>{content.runtime.call}</code>
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
              <div className="code-runtime-demo__complete">
                <CapabilityIcon name="check" /> {content.runtime.complete}
              </div>
              <div className="code-runtime-demo__footer">
                {content.runtime.footer}
              </div>
            </div>
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

            <div
              className="code-capability-demo code-context-demo"
              aria-hidden="true"
            >
              <div className="code-context-demo__query">
                <span>›</span>
                <code>{content.context.query}</code>
                <i />
              </div>
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
              <div className="code-context-demo__safety">
                <CapabilityIcon name="shield" /> {content.context.safety}
              </div>
            </div>
          </article>
        </div>
      </div>
    </section>
  );
}
