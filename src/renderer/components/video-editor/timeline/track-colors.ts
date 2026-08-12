export interface TrackColors {
  border: string;
  gradient: [string, string];
  selectedGradient: [string, string];
  cutMarker?: string;
}

export const TRACK_COLORS: Record<string, TrackColors> = {
  orange: {
    border: 'border-amber-600',
    gradient: ['#d97706', '#b45309'],
    selectedGradient: ['#3b82f6', '#1d4ed8'],
    cutMarker: 'bg-amber-500',
  },
  indigo: {
    border: 'border-indigo-600',
    gradient: ['#818cf8', '#4f46e5'],
    selectedGradient: ['#3b82f6', '#1d4ed8'],
  },
  purple: {
    border: 'border-purple-600',
    gradient: ['#c084fc', '#7e22ce'],
    selectedGradient: ['#3b82f6', '#1d4ed8'],
  },
  pink: {
    border: 'border-pink-600',
    gradient: ['#f472b6', '#be185d'],
    selectedGradient: ['#3b82f6', '#1d4ed8'],
  },
};

const SELECTED_GRADIENT: [string, string] = ['#3b82f6', '#1d4ed8'];

export const DRAWING_TRACK_COLORS: Record<string, TrackColors> = {
  pen: {
    border: 'border-teal-600',
    gradient: ['#2dd4bf', '#0f766e'],
    selectedGradient: SELECTED_GRADIENT,
  },
  highlight: {
    border: 'border-yellow-600',
    gradient: ['#facc15', '#a16207'],
    selectedGradient: SELECTED_GRADIENT,
  },
  rectangle: {
    border: 'border-sky-600',
    gradient: ['#38bdf8', '#0369a1'],
    selectedGradient: SELECTED_GRADIENT,
  },
  circle: {
    border: 'border-cyan-600',
    gradient: ['#22d3ee', '#0e7490'],
    selectedGradient: SELECTED_GRADIENT,
  },
  line: {
    border: 'border-lime-600',
    gradient: ['#a3e635', '#4d7c0f'],
    selectedGradient: SELECTED_GRADIENT,
  },
  arrow: {
    border: 'border-emerald-600',
    gradient: ['#34d399', '#047857'],
    selectedGradient: SELECTED_GRADIENT,
  },
  text: {
    border: 'border-pink-600',
    gradient: ['#f472b6', '#be185d'],
    selectedGradient: SELECTED_GRADIENT,
  },
  number: {
    border: 'border-orange-600',
    gradient: ['#fb923c', '#c2410c'],
    selectedGradient: SELECTED_GRADIENT,
  },
  redact: {
    border: 'border-rose-600',
    gradient: ['#fb7185', '#9f1239'],
    selectedGradient: SELECTED_GRADIENT,
  },
};

export function getDrawingTrackColors(type: string | undefined): TrackColors {
  if (type && type in DRAWING_TRACK_COLORS) {
    return DRAWING_TRACK_COLORS[type];
  }
  return DRAWING_TRACK_COLORS.pen;
}
