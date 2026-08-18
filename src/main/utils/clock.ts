export function formatClock(): string {
  const now = new Date();
  const part = (value: number, length: number) =>
    value.toString().padStart(length, '0');

  return `${part(now.getHours(), 2)}:${part(now.getMinutes(), 2)}:${part(
    now.getSeconds(),
    2
  )}.${part(now.getMilliseconds(), 3)}`;
}
