// Unexpected errors in the interface go to Tessera's local log (Settings >
// Diagnostics). The backend removes file and folder names before writing;
// nothing is sent anywhere.
import { api } from './api'

/** Records an unexpected error in the local log; never throws. */
export function logScreenError(error: unknown, extra = ''): void {
  try {
    const text = error instanceof Error ? `${error.name}: ${error.message}\n${error.stack ?? ''}` : String(error)
    void api.logError(`${text}${extra}`)?.catch(() => undefined)
  } catch {
    // Logging must never cause another error.
  }
}

/** Errors outside React's own handling (event handlers, promises). */
export function watchForErrors(): void {
  window.addEventListener('error', (e) => logScreenError(e.error ?? e.message))
  window.addEventListener('unhandledrejection', (e) => logScreenError(e.reason))
}
