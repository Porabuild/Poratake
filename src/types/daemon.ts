export interface DaemonRequest {
  id: string;
  module: string;
  method: string;
  params?: Record<string, unknown>;
}

export interface DaemonResponse {
  id: string;
  success: boolean;
  result?: unknown;
  error?: DaemonError;
}

export interface DaemonError {
  code: string;
  message: string;
}

export interface DaemonEvent {
  event: string;
  data?: unknown;
}

export type DaemonMessage = DaemonResponse | DaemonEvent;

export function isDaemonEvent(msg: DaemonMessage): msg is DaemonEvent {
  return 'event' in msg;
}

export function isDaemonResponse(msg: DaemonMessage): msg is DaemonResponse {
  return 'id' in msg && 'success' in msg;
}

export type DaemonEventHandler = (event: string, data: unknown) => void;

export interface PendingRequest {
  resolve: (result: unknown) => void;
  reject: (error: Error) => void;
  timeout: NodeJS.Timeout;
}

export type DaemonModule = keyof typeof DAEMON_METHODS;

export type DaemonMethod<TModule extends DaemonModule> =
  (typeof DAEMON_METHODS)[TModule][number];

export type DaemonCallArguments = {
  [TModule in DaemonModule]: [
    module: TModule,
    method: DaemonMethod<TModule>,
    params?: Record<string, unknown>,
    timeout?: number,
  ];
}[DaemonModule];
import { DAEMON_METHODS } from './daemon-methods.generated';

export { DAEMON_METHODS };
