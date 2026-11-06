// "Choose which" (prototype `partsHTML`): pick the projects, models,
// emulators, backups, apps or versions inside an item. The backend stores the
// choice; the item's size follows the selected parts.
import { gb } from "../../lib/format";
import type { Suggestion } from "../../lib/types";

export function Parts({ item, ctx, onChange }: { item: Suggestion; ctx: string; onChange: (partIds: string[]) => void }) {
  if (!item.parts.length) return null;
  const on = item.parts.filter((p) => p.selected).length;
  const toggle = (id: string, checked: boolean) => onChange(item.parts.filter((p) => (p.id === id ? checked : p.selected)).map((p) => p.id));
  return (
    <details className="parts" data-open={`${ctx}-parts-${item.id}`}>
      <summary>{`Choose which (${on} of ${item.parts.length})`}</summary>
      {item.parts.map((p, n) => {
        const id = `${ctx}-part-${item.id}-${n}`;
        return (
          <label className="part" htmlFor={id} key={p.id}>
            <input type="checkbox" id={id} data-part={`${item.id}:${n}`} checked={p.selected} onChange={(e) => toggle(p.id, e.target.checked)} />
            <span>{p.label}</span>
            <span className="num">{gb(p.sizeBytes)}</span>
          </label>
        );
      })}
    </details>
  );
}
