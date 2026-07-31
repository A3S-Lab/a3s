import type { CSSProperties, ReactNode } from "react";

type TypingStyle = CSSProperties & {
  "--typing-index": number;
};

function characterCount(value: string) {
  return Array.from(value).length;
}

function Character({ character, index }: { character: string; index: number }) {
  return (
    <span
      className="a3s-type-char"
      style={{ "--typing-index": index } as TypingStyle}
    >
      {character}
    </span>
  );
}

function TypedText({
  startIndex,
  text,
  preserveWords = false,
}: {
  startIndex: number;
  text: string;
  preserveWords?: boolean;
}) {
  if (!preserveWords || !/\s/.test(text)) {
    return Array.from(text).map((character, index) => (
      <Character
        character={character}
        index={startIndex + index}
        key={`${startIndex + index}-${character}`}
      />
    ));
  }

  let characterIndex = startIndex;
  const nodes: ReactNode[] = [];
  for (const [tokenIndex, token] of text.split(/(\s+)/).entries()) {
    const characters = Array.from(token);
    const tokenStart = characterIndex;
    characterIndex += characters.length;

    if (/^\s+$/.test(token)) {
      nodes.push(
        ...characters.map((character, index) => (
          <Character
            character={character}
            index={tokenStart + index}
            key={`${tokenStart + index}-space`}
          />
        )),
      );
      continue;
    }

    nodes.push(
      <span className="a3s-type-word" key={`${tokenIndex}-${token}`}>
        {characters.map((character, index) => (
          <Character
            character={character}
            index={tokenStart + index}
            key={`${tokenStart + index}-${character}`}
          />
        ))}
      </span>,
    );
  }
  return nodes;
}

export function HeroTypewriter({
  accent,
  description,
  id,
  lineOne,
  lineTwo,
}: {
  accent: string;
  description: string;
  id: string;
  lineOne: string;
  lineTwo: string;
}) {
  const lineTwoStart = characterCount(lineOne) + 5;
  const accentStart = lineTwoStart + characterCount(lineTwo) + 5;
  const descriptionStart = accentStart + characterCount(accent) + 14;

  return (
    <>
      <h1
        aria-label={`${lineOne} ${lineTwo} ${accent}`}
        className="a3s-hero__typed-title"
        id={id}
      >
        <span className="a3s-hero__typed-line" aria-hidden="true">
          <TypedText startIndex={0} text={lineOne} preserveWords />
        </span>
        <span className="a3s-hero__typed-line" aria-hidden="true">
          <TypedText startIndex={lineTwoStart} text={lineTwo} preserveWords />
        </span>
        <em className="a3s-hero__typed-line" aria-hidden="true">
          <TypedText startIndex={accentStart} text={accent} preserveWords />
          <i
            className="a3s-hero__type-caret"
            style={
              {
                "--typing-index": descriptionStart - 8,
              } as TypingStyle
            }
          />
        </em>
      </h1>
      <p aria-label={description} className="a3s-hero__description">
        <span aria-hidden="true">
          <TypedText
            startIndex={descriptionStart}
            text={description}
            preserveWords
          />
        </span>
      </p>
    </>
  );
}
