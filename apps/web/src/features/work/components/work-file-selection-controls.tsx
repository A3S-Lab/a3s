import { Check, Minus } from 'lucide-react';

export function WorkFileSelectionControl({ selected }: { selected: boolean }) {
  return (
    <span
      className={`work-file-selection-control ${selected ? 'checked' : ''}`}
      data-work-file-selection-control
      aria-hidden='true'
    >
      {selected && <Check size={12} strokeWidth={3} />}
    </span>
  );
}

export function WorkFilesSelectAllControl({
  selectedCount,
  totalCount,
  onSelectAll,
  onClear,
}: {
  selectedCount: number;
  totalCount: number;
  onSelectAll: () => void;
  onClear: () => void;
}) {
  const checked = totalCount > 0 && selectedCount === totalCount;
  const mixed = selectedCount > 0 && !checked;
  const label = checked ? `取消选择全部 ${totalCount} 项` : `选择全部 ${totalCount} 项`;

  return (
    <label className={`work-files-select-all-control ${checked || mixed ? 'checked' : ''}`} title={label}>
      <input
        type='checkbox'
        checked={checked}
        aria-checked={mixed ? 'mixed' : checked}
        aria-label={label}
        onChange={checked ? onClear : onSelectAll}
      />
      {mixed ? <Minus size={11} strokeWidth={3} /> : checked ? <Check size={11} strokeWidth={3} /> : null}
    </label>
  );
}
