import type { Dart, Ring } from '@/api/protocol'

/** Multiplier-style rings the operator can pick for a normal segment (1–20). */
export type Multiplier = 'Single' | 'Double' | 'Treble'

const MULTIPLIER_FACTOR: Record<Multiplier, number> = {
  Single: 1,
  Double: 2,
  Treble: 3,
}

/**
 * Build a Dart from a segment + multiplier, mirroring `crate::geometry::Score`
 * so the value the backend computes matches what we send for manual entry.
 */
export function makeDart(segment: number, multiplier: Multiplier): Dart {
  return {
    ring: multiplier,
    segment,
    value: segment * MULTIPLIER_FACTOR[multiplier],
  }
}

export function outerBull(): Dart {
  return { ring: 'OuterBull', segment: 25, value: 25 }
}

export function innerBull(): Dart {
  return { ring: 'InnerBull', segment: 25, value: 50 }
}

export function miss(): Dart {
  return { ring: 'Miss', segment: 0, value: 0 }
}

const RING_PREFIX: Partial<Record<Ring, string>> = {
  Single: 'S',
  Double: 'D',
  Treble: 'T',
}

/** Compact label, e.g. "T20", "D16", "25", "Bull", "—". */
export function dartLabel(dart: Dart): string {
  switch (dart.ring) {
    case 'Miss':
      return '—'
    case 'InnerBull':
      return 'Bull'
    case 'OuterBull':
      return '25'
    default:
      return `${RING_PREFIX[dart.ring] ?? ''}${dart.segment}`
  }
}

/** The standard dartboard segment order, clockwise from the top (20). */
export const BOARD_NUMBERS = [
  20, 1, 18, 4, 13, 6, 10, 15, 2, 17, 3, 19, 7, 16, 8, 11, 14, 9, 12, 5,
]
