export interface AspectRatio {
  name: string;
  width: number;
  height: number;
}

export const ASPECT_RATIOS: AspectRatio[] = [
  { name: 'Free', width: 0, height: 0 },
  { name: '16:9', width: 16, height: 9 },
  { name: '9:16', width: 9, height: 16 },
  { name: '4:3', width: 4, height: 3 },
  { name: '1:1', width: 1, height: 1 },
  { name: '21:9', width: 21, height: 9 },
  { name: '4:5', width: 4, height: 5 },
  { name: '3:2', width: 3, height: 2 },
];
