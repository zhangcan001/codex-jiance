import { useMemo } from "react";

export interface CountdownState {
  remainingSeconds: number | null;
  displayText: string;
  expired: boolean;
}

export function getRemainingSeconds(resetsAt: number | null, nowSeconds: number): number | null {
  return resetsAt === null ? null : Math.max(0, resetsAt - Math.floor(nowSeconds));
}

export function formatCountdown(remainingSeconds: number): string {
  const totalSeconds = Math.max(0, Math.floor(remainingSeconds));
  const seconds = totalSeconds % 60;
  const totalMinutes = Math.floor(totalSeconds / 60);
  const minutes = totalMinutes % 60;
  const totalHours = Math.floor(totalMinutes / 60);
  const hours = totalHours % 24;
  const days = Math.floor(totalHours / 24);
  const pad = (value: number) => String(value).padStart(2, "0");

  if (totalSeconds < 3600) {
    return `${pad(minutes)}:${pad(seconds)}`;
  }
  if (totalSeconds < 86400) {
    return `${pad(hours)}:${pad(minutes)}:${pad(seconds)}`;
  }
  return `${days}d ${pad(hours)}:${pad(minutes)}:${pad(seconds)}`;
}

export function getCountdownState(resetsAt: number | null, nowSeconds: number): CountdownState {
  const remainingSeconds = getRemainingSeconds(resetsAt, nowSeconds);

  return {
    remainingSeconds,
    displayText: remainingSeconds === null ? "Not reported" : formatCountdown(remainingSeconds),
    expired: remainingSeconds === 0,
  };
}

export function useCountdown(resetsAt: number | null, nowSeconds: number): CountdownState {
  return useMemo(() => getCountdownState(resetsAt, nowSeconds), [nowSeconds, resetsAt]);
}

export function shouldAutoRefreshReset(
  resetAt: number | null,
  nowSeconds: number,
  alreadyRefreshed: ReadonlySet<number>,
): boolean {
  return resetAt !== null && resetAt <= Math.floor(nowSeconds) && !alreadyRefreshed.has(resetAt);
}
