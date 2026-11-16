import { useState } from 'react';
import { createRoot } from 'react-dom/client';
import SvgAnnotationsOverlay from '@/renderer/components/editor/svg-annotations-overlay';
import type { Annotation } from '@/types/editor';

const annotations: Annotation[] = [
  {
    id: 'rectangle',
    type: 'rectangle',
    x: 20,
    y: 20,
    width: 35,
    height: 25,
    stroke: '#ff0000',
    strokeWidth: 2,
    fill: '#ff0000',
  },
  {
    id: 'circle',
    type: 'circle',
    x: 100,
    y: 30,
    radius: 15,
    stroke: '#ff0000',
    strokeWidth: 2,
    fill: '#ff0000',
  },
  {
    id: 'number',
    type: 'number',
    x: 190,
    y: 30,
    value: 1,
    displayValue: '1',
    fill: '#ff0000',
    size: 'medium',
  },
  {
    id: 'arrow',
    type: 'arrow',
    points: [20, 85, 60, 85],
    stroke: '#ff0000',
    strokeWidth: 4,
  },
  {
    id: 'line',
    type: 'line',
    points: [100, 85, 140, 85],
    stroke: '#ff0000',
    strokeWidth: 4,
  },
  {
    id: 'redact',
    type: 'redact',
    x: 170,
    y: 75,
    width: 40,
    height: 25,
    style: 'blackout',
    intensity: 5,
  },
  {
    id: 'text',
    type: 'text',
    x: 20,
    y: 120,
    text: 'Text',
    fontSize: 20,
    fill: '#ff0000',
    backgroundColor: '#000000',
  },
  {
    id: 'pen',
    type: 'pen',
    points: [20, 190, 35, 190, 50, 190, 60, 190],
    stroke: '#ff0000',
    strokeWidth: 5,
  },
  {
    id: 'highlight',
    type: 'highlight',
    points: [100, 190, 130, 190, 145, 190],
    fill: '#ffff00',
    opacity: 0.4,
    strokeWidth: 12,
  },
];

function Fixture() {
  const [selected, setSelected] = useState<string[]>([]);
  return (
    <>
      <div style={{ position: 'relative', width: 320, height: 240 }}>
        <SvgAnnotationsOverlay
          annotations={annotations}
          currentAnnotation={null}
          width={320}
          height={240}
          selectedAnnotationIds={selected}
          onSelect={id => setSelected(id ? [id] : [])}
          editingTextId={null}
          activeTool="select"
          showBlackoutFill
          hidePaint
        />
      </div>
      <button id="clear" onClick={() => setSelected([])}>
        Clear
      </button>
      <output id="selected">{selected.join(',')}</output>
    </>
  );
}

createRoot(document.querySelector('#root')!).render(<Fixture />);
