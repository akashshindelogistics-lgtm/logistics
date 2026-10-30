import { useCountUp } from '../lib/motion';

/** A number that counts up to `value` (instantly under reduced motion). */
export default function CountUp({ value }: { value: number }) {
  return <>{useCountUp(value).toLocaleString()}</>;
}
