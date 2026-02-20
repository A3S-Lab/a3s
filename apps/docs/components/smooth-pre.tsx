'use client';

import { InnerPre, getPreRef } from 'codehike/code';
import type { CustomPreProps } from 'codehike/code';
import {
  getStartingSnapshot,
  calculateTransitions,
  type TokenTransitionsSnapshot,
} from 'codehike/utils/token-transitions';
import React from 'react';

const DURATION = 600;

export class SmoothPre extends React.Component<CustomPreProps> {
  ref: React.RefObject<HTMLPreElement>;

  constructor(props: CustomPreProps) {
    super(props);
    this.ref = getPreRef(this.props);
  }

  render() {
    return <InnerPre merge={this.props} style={{ position: 'relative' }} />;
  }

  getSnapshotBeforeUpdate(): TokenTransitionsSnapshot {
    return getStartingSnapshot(this.ref.current!);
  }

  componentDidUpdate(
    _prevProps: never,
    _prevState: never,
    snapshot: TokenTransitionsSnapshot,
  ) {
    const transitions = calculateTransitions(this.ref.current!, snapshot);
    transitions.forEach(({ element, keyframes, options }) => {
      const { translateX, translateY, ...kf } = keyframes as any;
      if (translateX && translateY) {
        (kf as any).translate = [
          `${translateX[0]}px ${translateY[0]}px`,
          `${translateX[1]}px ${translateY[1]}px`,
        ];
      }
      element.animate(kf, {
        duration: options.duration * DURATION,
        delay: options.delay * DURATION,
        easing: options.easing,
        fill: options.fill,
      });
    });
  }
}
