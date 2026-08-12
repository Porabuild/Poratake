import { Slider } from '@/renderer/components/ui/slider';

interface SliderConfig {
  label: string;
  value: number;
  min: number;
  max: number;
  step?: number;
  onChange: (value: number) => void;
}

interface WallpaperSlidersProps {
  blur?: SliderConfig;
  noise?: SliderConfig;
  padding?: SliderConfig;
  corners?: SliderConfig;
  shadow?: SliderConfig;
}

function SliderControl({ config }: { config: SliderConfig }) {
  return (
    <div className="flex flex-col gap-3">
      <div className="flex items-center justify-between">
        <span className="text-xs font-medium">{config.label}</span>
        <span className="text-xs tabular-nums">{config.value}</span>
      </div>
      <Slider
        size="sm"
        value={[config.value]}
        onValueChange={([value]) => config.onChange(value)}
        min={config.min}
        max={config.max}
        step={config.step ?? 1}
      />
    </div>
  );
}

export default function WallpaperSliders({
  blur,
  noise,
  padding,
  corners,
  shadow,
}: WallpaperSlidersProps) {
  return (
    <div className="flex flex-col gap-4">
      {blur && <SliderControl config={blur} />}
      {noise && <SliderControl config={noise} />}
      {padding && <SliderControl config={padding} />}
      {corners && <SliderControl config={corners} />}
      {shadow && <SliderControl config={shadow} />}
    </div>
  );
}
