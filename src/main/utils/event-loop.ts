export function flushPendingContinuations(): void {
  setImmediate(() => {});
}
