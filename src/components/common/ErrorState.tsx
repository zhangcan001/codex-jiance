interface ErrorStateProps {
  message: string;
  title?: string;
  onRetry?: () => void;
}

export function ErrorState({
  message,
  title = "系统状态加载失败",
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
            重试
          </button>
        ) : null}
      </div>
    </div>
  );
}
