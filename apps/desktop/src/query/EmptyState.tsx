interface Props {
  onOpenSettings: () => void;
}

/** Brief 5.8: calm state shown when no provider is switched on. */
export function EmptyState({ onOpenSettings }: Props) {
  return (
    <div className="query-empty">
      <div className="query-empty-title">Connect a model to ask questions</div>
      <div className="query-empty-body">
        Local models are free. Commands, apps, files and projects work without AI.
      </div>
      <button type="button" className="query-btn" onClick={onOpenSettings}>
        AI providers
      </button>
    </div>
  );
}
