import type { AppModel } from "../App";
import { gb, trashWord } from "../lib/format";

/** Space held in Freeboard's Trash, with a separately confirmed empty step. */
export function TrashBox({ model }: { model: AppModel }) {
  const bytes = model.status.trashBytes;
  if (bytes <= 0) return null;
  const w = trashWord(model.status.platform);
  if (model.trashAsk)
    return (
      <div className="trash">
        <p>
          <b>{"Permanently delete what Freeboard moved to the " + w + "?"}</b>
          {" This frees " + gb(bytes) + " and cannot be undone."}
        </p>
        <div className="trash-actions">
          <button className="btn danger" data-action="empty-yes" onClick={() => model.emptyTrash()}>
            Delete permanently
          </button>
          <button className="btn" data-action="empty-no" onClick={() => model.setTrashAsk(false)}>
            Keep it
          </button>
        </div>
      </div>
    );
  return (
    <div className="trash">
      <p className="num">
        <b>{gb(bytes)}</b>
        {" is still held in the " + w + ". It does not count as free space yet."}
      </p>
      <div className="trash-actions">
        <button className="btn" data-action="empty-ask" onClick={() => model.setTrashAsk(true)}>
          {"Empty items from " + w}
        </button>
      </div>
    </div>
  );
}
