import { useState, useCallback, useRef } from 'react';

interface HistoryState<T> {
  past: T[];
  present: T;
  future: T[];
}

interface UseHistoryReturn<T> {
  state: T;
  set: (newPresent: T) => void;
  setWithoutHistory: (newPresent: T) => void;
  commitToHistory: () => void;
  undo: () => void;
  redo: () => void;
  canUndo: boolean;
  canRedo: boolean;
  reset: (newPresent: T) => void;
}

export const useHistory = <T>(initialState: T): UseHistoryReturn<T> => {
  const [state, setState] = useState<HistoryState<T>>({
    past: [],
    present: initialState,
    future: [],
  });

  const preChangeStateRef = useRef<T | null>(null);

  const canUndo = state.past.length > 0;
  const canRedo = state.future.length > 0;

  const set = useCallback((newPresent: T) => {
    setState(currentState => ({
      past: [...currentState.past, currentState.present].slice(-50),
      present: newPresent,
      future: [],
    }));
    preChangeStateRef.current = null;
  }, []);

  const setWithoutHistory = useCallback((newPresent: T) => {
    setState(currentState => {
      if (preChangeStateRef.current === null) {
        preChangeStateRef.current = currentState.present;
      }
      return {
        ...currentState,
        present: newPresent,
      };
    });
  }, []);

  const commitToHistory = useCallback(() => {
    if (preChangeStateRef.current !== null) {
      setState(currentState => ({
        past: [...currentState.past, preChangeStateRef.current!].slice(-50),
        present: currentState.present,
        future: [],
      }));
      preChangeStateRef.current = null;
    }
  }, []);

  const undo = useCallback(() => {
    setState(currentState => {
      if (currentState.past.length === 0) return currentState;

      const previous = currentState.past[currentState.past.length - 1];
      const newPast = currentState.past.slice(0, currentState.past.length - 1);

      return {
        past: newPast,
        present: previous,
        future: [currentState.present, ...currentState.future],
      };
    });
  }, []);

  const redo = useCallback(() => {
    setState(currentState => {
      if (currentState.future.length === 0) return currentState;

      const next = currentState.future[0];
      const newFuture = currentState.future.slice(1);

      return {
        past: [...currentState.past, currentState.present],
        present: next,
        future: newFuture,
      };
    });
  }, []);

  const reset = useCallback((newPresent: T) => {
    setState({
      past: [],
      present: newPresent,
      future: [],
    });
  }, []);

  return {
    state: state.present,
    set,
    setWithoutHistory,
    commitToHistory,
    undo,
    redo,
    canUndo,
    canRedo,
    reset,
  };
};
