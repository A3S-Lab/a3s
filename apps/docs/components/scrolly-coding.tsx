'use client';

import { Selection, Selectable, SelectionProvider } from 'codehike/utils/selection';
import { type ReactNode } from 'react';

export function ScrollyCoding({ children }: { children: ReactNode }) {
  return (
    <SelectionProvider className="flex gap-6 items-start py-6 not-prose">
      {children}
    </SelectionProvider>
  );
}

export function ScrollySteps({ children }: { children: ReactNode }) {
  return (
    <div className="flex-1 min-w-[220px] flex flex-col gap-3 py-2 mb-[50vh]">
      {children}
    </div>
  );
}

export function ScrollyStep({
  children,
  index,
}: {
  children: ReactNode;
  index: number;
}) {
  return (
    <Selectable
      index={index}
      selectOn={['click', 'scroll']}
      className="group relative pl-4 pr-5 py-4 rounded-lg cursor-pointer transition-all duration-200
        border border-fd-border/50
        bg-fd-card
        hover:border-fd-border
        hover:bg-fd-accent/50
        data-[selected=true]:border-fd-primary/60
        data-[selected=true]:bg-fd-primary/[0.06]
        data-[selected=true]:shadow-sm"
    >
      {/* left accent bar */}
      <span className="
        absolute left-0 top-3 bottom-3 w-[3px] rounded-full
        bg-fd-border
        transition-all duration-200
        group-data-[selected=true]:bg-fd-primary
      " />

      {/* step number */}
      <span className="
        absolute -top-2.5 left-3
        text-[10px] font-mono font-semibold px-1.5 py-0.5 rounded
        bg-fd-background border border-fd-border
        text-fd-muted-foreground
        group-data-[selected=true]:border-fd-primary/50
        group-data-[selected=true]:text-fd-primary
        transition-colors duration-200
      ">
        {String(index + 1).padStart(2, '0')}
      </span>

      <div className="
        pt-1
        [&_h3]:mt-0 [&_h3]:mb-1.5 [&_h3]:text-sm [&_h3]:font-semibold [&_h3]:text-fd-foreground
        [&_p]:text-xs [&_p]:text-fd-muted-foreground [&_p]:leading-relaxed [&_p]:mb-0
        [&_code]:text-[11px] [&_code]:bg-fd-muted [&_code]:text-fd-foreground [&_code]:px-1.5 [&_code]:py-0.5 [&_code]:rounded
      ">
        {children}
      </div>
    </Selectable>
  );
}

export function ScrollyCode({ children }: { children: ReactNode }) {
  return (
    <div className="
      w-[72%] shrink-0
      sticky top-20 self-start
      h-[calc(100vh-6rem)]
      flex flex-col
      rounded-xl
      border border-fd-border
      bg-[#282c34]
      shadow-2xl
      overflow-hidden
    ">
      {/* title bar */}
      <div className="
        flex items-center justify-between
        px-4 py-2.5
        border-b border-white/[0.07]
        bg-white/[0.02]
        shrink-0
      ">
        {/* traffic lights */}
        <div className="flex items-center gap-1.5">
          <span className="w-3 h-3 rounded-full bg-[#ff5f57] opacity-80" />
          <span className="w-3 h-3 rounded-full bg-[#febc2e] opacity-80" />
          <span className="w-3 h-3 rounded-full bg-[#28c840] opacity-80" />
        </div>
        {/* decorative dots */}
        <div className="flex gap-1">
          <span className="w-1 h-1 rounded-full bg-white/10" />
          <span className="w-1 h-1 rounded-full bg-white/10" />
          <span className="w-1 h-1 rounded-full bg-white/10" />
        </div>
      </div>

      {/* code area — scrollable */}
      <div className="flex-1 min-h-0 overflow-y-auto overflow-x-auto ch-code-scroll-parent">
        <div>
          <Selection from={children as ReactNode[]} />
        </div>
      </div>
    </div>
  );
}
