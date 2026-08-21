import type { Point, Rect, Size } from '@/types/geometry';

export const MIN_SELECTION_SIZE = 10;
export const MIN_RESIZE_SIZE = 20;

const HANDLE_CORNER = 16;
const HANDLE_EDGE = 12;

export type SelectionHandle =
  | 'top-left'
  | 'top'
  | 'top-right'
  | 'right'
  | 'bottom-right'
  | 'bottom'
  | 'bottom-left'
  | 'left';

interface Edges {
  left: number;
  top: number;
  right: number;
  bottom: number;
}

const CURSORS: Record<SelectionHandle, string> = {
  'top-left': 'nwse-resize',
  'bottom-right': 'nwse-resize',
  'top-right': 'nesw-resize',
  'bottom-left': 'nesw-resize',
  top: 'ns-resize',
  bottom: 'ns-resize',
  left: 'ew-resize',
  right: 'ew-resize',
};

const LEFT_HANDLES: ReadonlySet<SelectionHandle> = new Set([
  'top-left',
  'left',
  'bottom-left',
]);
const RIGHT_HANDLES: ReadonlySet<SelectionHandle> = new Set([
  'top-right',
  'right',
  'bottom-right',
]);
const TOP_HANDLES: ReadonlySet<SelectionHandle> = new Set([
  'top-left',
  'top',
  'top-right',
]);
const BOTTOM_HANDLES: ReadonlySet<SelectionHandle> = new Set([
  'bottom-left',
  'bottom',
  'bottom-right',
]);

function toEdges(rect: Rect): Edges {
  return {
    left: rect.x,
    top: rect.y,
    right: rect.x + rect.width,
    bottom: rect.y + rect.height,
  };
}

function toRect(edges: Edges): Rect {
  return {
    x: edges.left,
    y: edges.top,
    width: edges.right - edges.left,
    height: edges.bottom - edges.top,
  };
}

function within(edges: Edges, point: Point): boolean {
  return (
    point.x >= edges.left &&
    point.x <= edges.right &&
    point.y >= edges.top &&
    point.y <= edges.bottom
  );
}

export function clampPoint(point: Point, bounds: Size): Point {
  return {
    x: Math.min(Math.max(point.x, 0), bounds.width),
    y: Math.min(Math.max(point.y, 0), bounds.height),
  };
}

export function normalizeRect(first: Point, second: Point): Rect {
  return toRect({
    left: Math.min(first.x, second.x),
    top: Math.min(first.y, second.y),
    right: Math.max(first.x, second.x),
    bottom: Math.max(first.y, second.y),
  });
}

export function containsPoint(rect: Rect, point: Point): boolean {
  return within(toEdges(rect), point);
}

export function isUsableSelection(rect: Rect): boolean {
  return rect.width > MIN_SELECTION_SIZE && rect.height > MIN_SELECTION_SIZE;
}

export function hitTestHandle(
  rect: Rect,
  point: Point
): SelectionHandle | null {
  const edges = toEdges(rect);
  const centerX = (edges.left + edges.right) / 2;
  const centerY = (edges.top + edges.bottom) / 2;
  const span = HANDLE_CORNER * 2;

  const tests: [SelectionHandle, Edges][] = [
    [
      'top-left',
      {
        left: edges.left - HANDLE_EDGE,
        top: edges.top - HANDLE_EDGE,
        right: edges.left + HANDLE_CORNER,
        bottom: edges.top + HANDLE_CORNER,
      },
    ],
    [
      'top-right',
      {
        left: edges.right - HANDLE_CORNER,
        top: edges.top - HANDLE_EDGE,
        right: edges.right + HANDLE_EDGE,
        bottom: edges.top + HANDLE_CORNER,
      },
    ],
    [
      'bottom-right',
      {
        left: edges.right - HANDLE_CORNER,
        top: edges.bottom - HANDLE_CORNER,
        right: edges.right + HANDLE_EDGE,
        bottom: edges.bottom + HANDLE_EDGE,
      },
    ],
    [
      'bottom-left',
      {
        left: edges.left - HANDLE_EDGE,
        top: edges.bottom - HANDLE_CORNER,
        right: edges.left + HANDLE_CORNER,
        bottom: edges.bottom + HANDLE_EDGE,
      },
    ],
    [
      'top',
      {
        left: centerX - span,
        top: edges.top - HANDLE_EDGE,
        right: centerX + span,
        bottom: edges.top + HANDLE_EDGE,
      },
    ],
    [
      'right',
      {
        left: edges.right - HANDLE_EDGE,
        top: centerY - span,
        right: edges.right + HANDLE_EDGE,
        bottom: centerY + span,
      },
    ],
    [
      'bottom',
      {
        left: centerX - span,
        top: edges.bottom - HANDLE_EDGE,
        right: centerX + span,
        bottom: edges.bottom + HANDLE_EDGE,
      },
    ],
    [
      'left',
      {
        left: edges.left - HANDLE_EDGE,
        top: centerY - span,
        right: edges.left + HANDLE_EDGE,
        bottom: centerY + span,
      },
    ],
  ];

  return tests.find(([, box]) => within(box, point))?.[0] ?? null;
}

export function cursorFor(rect: Rect | null, point: Point): string {
  if (!rect) {
    return 'crosshair';
  }

  const handle = hitTestHandle(rect, point);
  if (handle) {
    return CURSORS[handle];
  }

  return containsPoint(rect, point) ? 'move' : 'crosshair';
}

export function adjustRectToRatio(
  rect: Rect,
  ratio: number,
  handle: SelectionHandle | null
): Rect {
  if (ratio <= 0 || rect.width <= 0 || rect.height <= 0) {
    return rect;
  }

  const edges = toEdges(rect);

  if (rect.width / rect.height > ratio) {
    const width = Math.round(rect.height * ratio);

    if (!handle) {
      const center = (edges.left + edges.right) / 2;
      const left = Math.round(center - width / 2);
      return toRect({ ...edges, left, right: left + width });
    }

    return LEFT_HANDLES.has(handle)
      ? toRect({ ...edges, left: edges.right - width })
      : toRect({ ...edges, right: edges.left + width });
  }

  const height = Math.round(rect.width / ratio);

  if (!handle) {
    const center = (edges.top + edges.bottom) / 2;
    const top = Math.round(center - height / 2);
    return toRect({ ...edges, top, bottom: top + height });
  }

  return TOP_HANDLES.has(handle)
    ? toRect({ ...edges, top: edges.bottom - height })
    : toRect({ ...edges, bottom: edges.top + height });
}

export function fitRect(rect: Rect, bounds: Size): Rect {
  const width = Math.min(Math.max(rect.width, 1), bounds.width);
  const height = Math.min(Math.max(rect.height, 1), bounds.height);

  return {
    width,
    height,
    x: Math.min(Math.max(rect.x, 0), bounds.width - width),
    y: Math.min(Math.max(rect.y, 0), bounds.height - height),
  };
}

export function resizeRect(
  rect: Rect,
  point: Point,
  handle: SelectionHandle,
  ratio: number | null
): Rect {
  const edges = toEdges(rect);
  const resized = { ...edges };

  if (LEFT_HANDLES.has(handle)) {
    resized.left = Math.min(point.x, edges.right - MIN_RESIZE_SIZE);
  }

  if (RIGHT_HANDLES.has(handle)) {
    resized.right = Math.max(point.x, edges.left + MIN_RESIZE_SIZE);
  }

  if (TOP_HANDLES.has(handle)) {
    resized.top = Math.min(point.y, edges.bottom - MIN_RESIZE_SIZE);
  }

  if (BOTTOM_HANDLES.has(handle)) {
    resized.bottom = Math.max(point.y, edges.top + MIN_RESIZE_SIZE);
  }

  const next = toRect(resized);

  return ratio ? adjustRectToRatio(next, ratio, handle) : next;
}

export function moveRect(
  rect: Rect,
  point: Point,
  offset: Point,
  bounds: Size
): Rect {
  return {
    ...rect,
    x:
      rect.width >= bounds.width
        ? 0
        : Math.min(Math.max(point.x - offset.x, 0), bounds.width - rect.width),
    y:
      rect.height >= bounds.height
        ? 0
        : Math.min(
            Math.max(point.y - offset.y, 0),
            bounds.height - rect.height
          ),
  };
}
