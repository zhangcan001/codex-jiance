import type { ReactNode } from "react";

interface MetricCardProps {
  title: string;
  value: ReactNode;
  subtitle: string;
}

export function MetricCard({ title, value, subtitle }: MetricCardProps) {
  return (
    <article className="metric-card">
      <p className="metric-card__title">{title}</p>
      <p className="metric-card__value">{value}</p>
      <p className="metric-card__subtitle">{subtitle}</p>
    </article>
  );
}
