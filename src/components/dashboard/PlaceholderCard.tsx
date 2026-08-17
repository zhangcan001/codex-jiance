interface PlaceholderCardProps {
  title: string;
  value: string;
  subtitle: string;
}

export function PlaceholderCard({ title, value, subtitle }: PlaceholderCardProps) {
  return (
    <article className="metric-card metric-card--placeholder">
      <div className="metric-card__placeholder-mark" aria-hidden="true">
        ···
      </div>
      <p className="metric-card__title">{title}</p>
      <p className="metric-card__value">{value}</p>
      <p className="metric-card__subtitle">{subtitle}</p>
    </article>
  );
}
