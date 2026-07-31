import { useLang } from "@rspress/core/runtime";
import HomePage from "../../components/home-page";
import { homeContent, type Lang } from "../../components/home/home-content";

function MarkdownHome({ lang }: { lang: Lang }) {
  const content = homeContent[lang];

  return (
    <main>
      <h1>
        {content.hero.lineOne} {content.hero.lineTwo} {content.hero.accent}
      </h1>
      <p>{content.hero.description}</p>
      <h2>{content.aiNative.title}</h2>
      <p>{content.aiNative.description}</p>
      <h3>{content.aiNative.interactionTitle}</h3>
      <p>{content.aiNative.interactionDescription}</p>
      {content.aiNative.steps.map((step) => (
        <section key={step.id}>
          <h4>
            {step.index} {step.title}
          </h4>
          <p>{step.detail}</p>
        </section>
      ))}
      <h3>{content.aiNative.organizationTitle}</h3>
      <p>{content.aiNative.organizationDescription}</p>
      {content.aiNative.reasons.map((reason) => (
        <section key={reason.index}>
          <h4>{reason.title}</h4>
          <p>{reason.description}</p>
        </section>
      ))}
      <h2>{content.architecture.title}</h2>
      <p>{content.architecture.description}</p>
    </main>
  );
}

export function HomeLayout() {
  const lang: Lang = useLang() === "en" ? "en" : "cn";

  if (import.meta.env.SSG_MD) {
    return <MarkdownHome lang={lang} />;
  }

  return <HomePage lang={lang} />;
}
