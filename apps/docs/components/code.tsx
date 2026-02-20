import { Pre, highlight, InnerToken } from 'codehike/code';
import type { RawCode, AnnotationHandler } from 'codehike/code';
import { SmoothPre } from './smooth-pre';

const tokenTransitions: AnnotationHandler = {
  name: 'token-transitions',
  PreWithRef: SmoothPre,
  Token: (props) => (
    <InnerToken merge={props} style={{ display: 'inline-block' }} />
  ),
};

export async function Code({ codeblock }: { codeblock: RawCode }) {
  const highlighted = await highlight(codeblock, 'one-dark-pro');
  return (
    // .ch-standalone: styled like a fumadocs Shiki block (border + radius + dark bg)
    // Inside ScrollyCode the parent sets bg/border, so we strip it via .ch-inline-code
    <div className="ch-codeblock not-prose">
      {codeblock.meta && (
        <div className="ch-codeblock-title">
          {codeblock.meta}
        </div>
      )}
      <Pre
        code={highlighted}
        handlers={[tokenTransitions]}
        className="ch-codeblock-pre"
      />
    </div>
  );
}
