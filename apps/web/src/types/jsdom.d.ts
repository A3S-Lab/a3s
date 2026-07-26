declare module 'jsdom' {
  export interface ConstructorOptions {
    url?: string;
  }

  export class JSDOM {
    readonly window: Window & typeof globalThis;

    constructor(html?: string, options?: ConstructorOptions);
  }
}
