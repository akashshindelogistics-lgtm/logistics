import type { ReactNode } from 'react';
import { LazyMotion, MotionConfig, domMax } from 'motion/react';

/**
 * App-wide motion setup. `LazyMotion strict` means components use the slim
 * `m.*` elements (importing `motion.*` would throw), and `domMax` adds the
 * layout animations the sliding sidebar highlight needs. `reducedMotion="user"`
 * makes every motion/react animation respect the OS "reduce motion" setting.
 */
export default function MotionProvider({ children }: { children: ReactNode }) {
  return (
    <LazyMotion features={domMax} strict>
      <MotionConfig reducedMotion="user">{children}</MotionConfig>
    </LazyMotion>
  );
}
