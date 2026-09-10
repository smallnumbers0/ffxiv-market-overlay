import { CloseIcon, PlusIcon } from "./icons";
import type { Board } from "../lib/tauriApi";

interface BoardStripProps {
  boards: Board[];
  canAddBoard: boolean;
  /** Which board the settings panel is currently editing, if it is open. */
  editing: string | null;
  onEdit: (id: string) => void;
  onRemove: (id: string) => void;
  onAdd: () => void;
}

/**
 * The boards being compared. Unlike a tab strip nothing here is "selected" for
 * viewing - every board is on screen as its own column below. This is where
 * you add, remove, and re-point them.
 */
export function BoardStrip({
  boards,
  canAddBoard,
  editing,
  onEdit,
  onRemove,
  onAdd,
}: BoardStripProps) {
  return (
    <div className="boardstrip">
      <div className="boardstrip-scroll">
        {boards.map((board) => (
          <div
            key={board.id}
            className={`board-chip${editing === board.id ? " is-editing" : ""}`}
          >
            <button
              type="button"
              className="board-chip-label"
              onClick={() => onEdit(board.id)}
              title={`Change the world or data center for ${
                board.scope ?? "this column"
              }`}
            >
              {board.scope ?? "Pick a world"}
            </button>
            {/* The last board has no close button: an overlay with no board
                to show is just an empty window. */}
            {boards.length > 1 && (
              <button
                type="button"
                className="board-chip-close"
                onClick={() => onRemove(board.id)}
                title={`Stop comparing ${board.scope ?? "this board"}`}
                aria-label={`Stop comparing ${board.scope ?? "this board"}`}
              >
                <CloseIcon size={10} />
              </button>
            )}
          </div>
        ))}
      </div>

      <button
        type="button"
        className="icon-button boardstrip-add"
        onClick={onAdd}
        disabled={!canAddBoard}
        title={
          canAddBoard
            ? "Compare another board, one step wider than the last"
            : "That's as many boards as fit side by side"
        }
        aria-label="Compare another board"
      >
        <PlusIcon />
      </button>
    </div>
  );
}
