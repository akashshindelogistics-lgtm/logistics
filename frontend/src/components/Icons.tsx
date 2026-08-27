interface IconProps { size?: number; className?: string }

const i = (d: string, opts?: { fill?: boolean }) =>
  ({ size = 18, className }: IconProps) => (
    <svg width={size} height={size} viewBox="0 0 24 24" fill={opts?.fill ? 'currentColor' : 'none'}
      stroke={opts?.fill ? 'none' : 'currentColor'} strokeWidth="1.75" strokeLinecap="round"
      strokeLinejoin="round" className={className}>
      <path d={d} />
    </svg>
  );

export const IconGrid     = i('M3 3h7v7H3zm11 0h7v7h-7zM3 14h7v7H3zm11 0h7v7h-7z');
export const IconBuilding = i('M6 2h12a2 2 0 0 1 2 2v18H4V4a2 2 0 0 1 2-2zm4 18v-6h4v6M9 6h1m5 0h1M9 10h1m5 0h1');
export const IconTruck    = i('M1 3h15v13H1zm15 5 3 3v5h-3zm-9 9a2 2 0 1 0 0 4 2 2 0 0 0 0-4zm9 2a2 2 0 1 0 0 4 2 2 0 0 0 0-4z');
export const IconUsers    = i('M17 21v-2a4 4 0 0 0-4-4H5a4 4 0 0 0-4 4v2m8-10a4 4 0 1 0 0-8 4 4 0 0 0 0 8zm10 10v-2a4 4 0 0 0-3-3.87m-4-12a4 4 0 0 1 0 7.75');
export const IconPackage  = i('M16.5 9.4 7.55 4.24M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 2 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16zM3.27 6.96 12 12.01l8.73-5.05M12 22.08V12');
export const IconPin      = i('M21 10c0 7-9 13-9 13s-9-6-9-13a9 9 0 0 1 18 0zm-9 3a3 3 0 1 0 0-6 3 3 0 0 0 0 6z');
export const IconTrash    = i('M3 6h18m-2 0V4a1 1 0 0 0-1-1H6a1 1 0 0 0-1 1v2m3 0v14a1 1 0 0 0 1 1h8a1 1 0 0 0 1-1V6H7z');
export const IconPlus     = i('M12 5v14M5 12h14');
export const IconX        = i('M18 6 6 18M6 6l12 12');
export const IconChevron  = i('M9 18l6-6-6-6');
export const IconDispatch = i('M22 2 11 13M22 2l-7 20-4-9-9-4 20-7z');
export const IconCheck    = i('M20 6 9 17l-5-5');
export const IconSearch   = i('M21 21l-6-6m2-5a7 7 0 1 1-14 0 7 7 0 0 1 14 0z');
export const IconClock    = i('M12 2a10 10 0 1 0 0 20A10 10 0 0 0 12 2zm0 5v5l4 2');
