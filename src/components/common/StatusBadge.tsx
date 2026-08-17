import type { ReactNode } from "react";

export type StatusVariant = "success" | "warning" | "error" | "neutral";

interface StatusBadgeProps {
  children: ReactNode;
  variant: StatusVariant;
}

export function StatusBadge({ children, variant }: StatusBadgeProps) {
  return <span className={`status-badge status-badge--${variant}`}>{children}</span>;
}
