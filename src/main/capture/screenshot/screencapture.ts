import { execFile } from 'child_process';

let activeInteractiveCapture: Promise<string> | null = null;

export function runScreencapture(args: string[]): Promise<string> {
  return new Promise((resolve, reject) => {
    execFile('screencapture', args, (error, _stdout, stderr) => {
      if (error) {
        reject(error);
        return;
      }

      resolve(stderr);
    });
  });
}

export function startInteractiveScreencapture(
  args: string[]
): Promise<string> | null {
  if (activeInteractiveCapture) {
    return null;
  }

  const capture = runScreencapture(args);
  activeInteractiveCapture = capture;

  return capture.finally(() => {
    if (activeInteractiveCapture === capture) {
      activeInteractiveCapture = null;
    }
  });
}
