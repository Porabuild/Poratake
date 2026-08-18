import { useState, useRef, useCallback } from 'react';
import type { Annotation } from '@/types/editor';

interface UseTextEditingProps {
  annotations: Annotation[];
  onAnnotationUpdate?: (id: string, updates: Partial<Annotation>) => void;
  onAnnotationDelete?: (id: string) => void;
  selectedColor: string;
}

export const useTextEditing = ({
  annotations,
  onAnnotationUpdate,
  onAnnotationDelete,
  selectedColor,
}: UseTextEditingProps) => {
  const [editingTextId, setEditingTextId] = useState<string | null>(null);
  const [textEditValue, setTextEditValue] = useState('');
  const [textEditPosition, setTextEditPosition] = useState<{
    x: number;
    y: number;
  } | null>(null);

  const isTextEditingRef = useRef(false);
  const hasTypedRef = useRef(false);

  const startTextEditing = useCallback(
    (
      pos: { x: number; y: number },
      existingTextId?: string,
      offset: { x: number; y: number } = { x: 0, y: 0 }
    ) => {
      if (existingTextId) {
        const textAnn = annotations.find(
          a => a.id === existingTextId && a.type === 'text'
        );

        if (textAnn && textAnn.type === 'text' && textAnn.text) {
          isTextEditingRef.current = true;
          hasTypedRef.current = true;
          setEditingTextId(existingTextId);
          setTextEditValue(textAnn.text);

          setTextEditPosition({
            x: textAnn.x + offset.x,
            y: textAnn.y + offset.y,
          });
        } else {
          isTextEditingRef.current = true;
          hasTypedRef.current = false;
          setEditingTextId(existingTextId);
          setTextEditValue('');

          setTextEditPosition({
            x: pos.x + offset.x,
            y: pos.y + offset.y,
          });
        }
      } else {
        const textAnnotation: Annotation = {
          id: `text-${Date.now()}`,
          type: 'text',
          x: pos.x,
          y: pos.y,
          text: '',
          fontSize: 20,
          fill: selectedColor,
        };

        isTextEditingRef.current = true;
        hasTypedRef.current = false;
        setEditingTextId(textAnnotation.id);
        setTextEditValue('');

        setTextEditPosition({
          x: pos.x + offset.x,
          y: pos.y + offset.y,
        });

        return textAnnotation;
      }
    },
    [annotations, selectedColor]
  );

  const finishTextEditing = useCallback(() => {
    if (!isTextEditingRef.current) return;

    if (hasTypedRef.current) {
      if (editingTextId && onAnnotationUpdate) {
        if (textEditValue.trim()) {
          onAnnotationUpdate(editingTextId, {
            text: textEditValue,
          } as Partial<Annotation>);
        } else if (onAnnotationDelete) {
          onAnnotationDelete(editingTextId);
        }
      }
    } else {
      if (editingTextId && onAnnotationDelete) {
        onAnnotationDelete(editingTextId);
      }
    }

    isTextEditingRef.current = false;
    hasTypedRef.current = false;
    setEditingTextId(null);
    setTextEditValue('');
    setTextEditPosition(null);
  }, [editingTextId, textEditValue, onAnnotationUpdate, onAnnotationDelete]);

  const handleTextChange = useCallback((value: string) => {
    setTextEditValue(value);
    hasTypedRef.current = true;
  }, []);

  return {
    editingTextId,
    textEditValue,
    textEditPosition,
    startTextEditing,
    finishTextEditing,
    handleTextChange,
  };
};
