import * as React from 'react';
import { Slider as HeroSlider } from '@heroui/react';

import { cn } from '@/renderer/lib/utils';

interface SliderProps extends Omit<
  React.ComponentProps<typeof HeroSlider>,
  | 'value'
  | 'defaultValue'
  | 'onChange'
  | 'isDisabled'
  | 'children'
  | 'minValue'
  | 'maxValue'
> {
  value?: number[];
  defaultValue?: number[];
  onValueChange?: (value: number[]) => void;
  disabled?: boolean;
  min?: number;
  max?: number;
}

function Slider({
  className,
  defaultValue,
  value,
  onValueChange,
  disabled,
  min = 0,
  max = 100,
  ...props
}: SliderProps) {
  const values = value ?? defaultValue ?? [min];

  return (
    <HeroSlider
      data-slot="slider"
      value={value}
      defaultValue={defaultValue}
      onChange={next => onValueChange?.(Array.isArray(next) ? next : [next])}
      isDisabled={disabled}
      minValue={min}
      maxValue={max}
      className={cn('w-full', className)}
      {...props}
    >
      <HeroSlider.Track>
        <HeroSlider.Fill />
        {values.map((_, index) => (
          <HeroSlider.Thumb key={index} index={index} />
        ))}
      </HeroSlider.Track>
    </HeroSlider>
  );
}

export { Slider };
