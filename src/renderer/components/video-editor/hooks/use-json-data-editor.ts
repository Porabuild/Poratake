import { useCallback, useMemo, useState } from 'react';
import type { ChangeEvent } from 'react';

export interface JsonDataValidation<T> {
  valid: boolean;
  error?: string;
  data?: T;
}

export interface JsonDataSaveResult {
  success: boolean;
  error?: string;
}

export interface JsonDataEditorOptions<T> {
  initialData: T | null;
  buildTemplate: () => string;
  example: string;
  validate: (parsed: unknown) => JsonDataValidation<T>;
  invalidMessage: string;
  onSave: (data: T) => Promise<JsonDataSaveResult>;
  onOpenChange: (open: boolean) => void;
}

export function useJsonDataEditor<T>({
  initialData,
  buildTemplate,
  example,
  validate,
  invalidMessage,
  onSave,
  onOpenChange,
}: JsonDataEditorOptions<T>) {
  const defaultValue = useMemo(() => {
    if (initialData) {
      return JSON.stringify(initialData, null, 2);
    }
    return buildTemplate();
  }, [initialData, buildTemplate]);

  const [value, setValue] = useState(defaultValue);
  const [error, setError] = useState<string | null>(null);
  const [isSaving, setIsSaving] = useState(false);

  const handleValueChange = useCallback(
    (e: ChangeEvent<HTMLTextAreaElement>) => {
      setValue(e.target.value);
      setError(null);
    },
    []
  );

  const handleSave = useCallback(async () => {
    try {
      const parsed = JSON.parse(value);
      const validation = validate(parsed);

      if (!validation.valid || !validation.data) {
        setError(validation.error ?? invalidMessage);
        return;
      }

      setIsSaving(true);
      const result = await onSave(validation.data);

      if (!result.success) {
        setError(result.error ?? 'Failed to save');
        return;
      }

      onOpenChange(false);
    } catch (e) {
      setError(e instanceof SyntaxError ? 'Invalid JSON syntax' : String(e));
    } finally {
      setIsSaving(false);
    }
  }, [value, validate, invalidMessage, onSave, onOpenChange]);

  const handleLoadExample = useCallback(() => {
    setValue(example);
    setError(null);
  }, [example]);

  const handleLoadTemplate = useCallback(() => {
    setValue(buildTemplate());
    setError(null);
  }, [buildTemplate]);

  return {
    value,
    error,
    isSaving,
    handleValueChange,
    handleSave,
    handleLoadExample,
    handleLoadTemplate,
  };
}
