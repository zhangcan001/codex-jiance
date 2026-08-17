interface ErrorStateProps {
  message: string;
  title?: string;
  onRetry?: () => void;
}

export function ErrorState({
  message,
  title = "Failed to load system status",
  onRetry,
}: ErrorStateProps) {
  return (
    <div className="error-state" role="alert">
      <div className="error-state__icon" aria-hidden="true">
        !
      </div>
      <div>
        <h3>{title}</h3>
        <p>{message}</p>
        {onRetry ? (
          <button className="button button--secondary" type="button" onClick={onRetry}>
            Retry
          </button>
        ) : null}
      </div>
    </div>
  );
}
