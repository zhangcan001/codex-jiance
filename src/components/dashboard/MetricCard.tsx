import type { ReactNode } from "react";

interface MetricCardProps {
  title: string;
  value: ReactNode;
  subtitle: string;
  label?: string;
}

export function MetricCard({ title, value, subtitle, label }: MetricCardProps) {
  return (
    <article className="metric-card">
      <p className="metric-card__title">{title}</p>
      {label ? <p className="metric-card__label">{label}</p> : null}
      <p className="metric-card__value">{value}</p>
      <p className="metric-card__subtitle">{subtitle}</p>
    </article>
  );
}
