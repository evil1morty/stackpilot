/**
 * setInterval wrapper that pauses while the document is hidden (window
 * minimised to tray, tab in background) and resumes — with one immediate
 * fire — when it comes back. Cuts CPU + IPC + kernel TCP-table enumeration
 * to zero while the user isn't looking.
 *
 * Returned function tears down the timer and the visibilitychange listener.
 * Call it from $effect cleanup or onDestroy.
 */
export function startPolling(fn: () => void, intervalMs: number): () => void {
  let handle: ReturnType<typeof setInterval> | null = null;
  let stopped = false;

  function start() {
    if (handle != null || stopped) return;
    handle = setInterval(() => {
      if (document.visibilityState === "visible") fn();
    }, intervalMs);
  }

  function stop() {
    if (handle != null) {
      clearInterval(handle);
      handle = null;
    }
  }

  function onVisibility() {
    if (document.visibilityState === "visible") {
      // Catch up immediately on resume so the user doesn't see stale data.
      fn();
      start();
    } else {
      stop();
    }
  }

  document.addEventListener("visibilitychange", onVisibility);

  if (document.visibilityState === "visible") start();

  return () => {
    stopped = true;
    stop();
    document.removeEventListener("visibilitychange", onVisibility);
  };
}
