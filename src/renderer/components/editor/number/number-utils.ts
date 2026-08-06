import type { Annotation, NumberStyle } from '@/types/editor';

const toRoman = (num: number): string => {
  const romanNumerals: [number, string][] = [
    [1000, 'M'],
    [900, 'CM'],
    [500, 'D'],
    [400, 'CD'],
    [100, 'C'],
    [90, 'XC'],
    [50, 'L'],
    [40, 'XL'],
    [10, 'X'],
    [9, 'IX'],
    [5, 'V'],
    [4, 'IV'],
    [1, 'I'],
  ];

  let result = '';
  let remaining = num;

  for (const [value, numeral] of romanNumerals) {
    while (remaining >= value) {
      result += numeral;
      remaining -= value;
    }
  }

  return result;
};

const toAlpha = (num: number, uppercase: boolean): string => {
  const letters = [];
  let remaining = num;

  while (remaining > 0) {
    remaining--;
    letters.unshift(String.fromCharCode((remaining % 26) + 65));
    remaining = Math.floor(remaining / 26);
  }

  const result = letters.join('');
  return uppercase ? result : result.toLowerCase();
};

export const getDisplayValue = (value: number, style: NumberStyle): string => {
  switch (style) {
    case 'roman':
      return toRoman(value);
    case 'alpha-upper':
      return toAlpha(value, true);
    case 'alpha-lower':
      return toAlpha(value, false);
    case 'numeric':
    default:
      return String(value);
  }
};

export const renumberAnnotations = (
  annotations: Annotation[],
  numberStyle: NumberStyle,
  startValue: number = 1
): Annotation[] => {
  let currentValue = startValue;

  return annotations.map(ann => {
    if (ann.type === 'number') {
      const newValue = currentValue;
      currentValue++;
      return {
        ...ann,
        value: newValue,
        displayValue: getDisplayValue(newValue, numberStyle),
      };
    }
    return ann;
  });
};

export const getNextNumberValue = (
  annotations: Annotation[],
  startValue: number = 1
): number => {
  const numberCount = annotations.filter(ann => ann.type === 'number').length;
  return startValue + numberCount;
};
